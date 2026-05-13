# Stage 12 -- Discussion Record

Distilled record of the design dialogue producing the
stage-12 Item-level vocabulary spec. Each entry captures
the question raised, key options considered, the decision,
the reasoning chain, methodology rules invoked, prior-art
evidence, and cross-references to `spec-snapshot.md` (the
spec) and `review-log.md` (the round-by-round audit
trail; created on first review round).

Status of decisions in this file mirrors `spec-snapshot.md`'s
"Item" section (§8) plus adjacent in-place updates to §3
(root struct + §3.1 invariant 10 reword + §3.3 Phase B
step 3 reordering + new §3.4 bridge construction-pipeline
order), §9.4 (InstanceKind::Function under Option B;
InstanceKind::Static under D-12-1.2 (c)), §14 (FnSig
location reframe per Option B), §18.2 (per-method
ownership row reframing per D-12-6 + D-12-K), §23.6.2
(callee_fn_sig helper drops Item indirection), and
stage-11 territory in §25.4 / §25.5 (FnSig + Body
location reframe per Option B).

D-tag convention: prefixed `D-12-K` form per `../README.md`
§5.2.

Status tags:
- `[LOCKED]` -- audit-shape recorded; per-stage Phase A
  closed; ready for Phase B Codex review.
- `[REOPENED]` -- a later sub-decision or sibling D-tag
  forced a revision; superseded entry kept for history.
- `[DEFERRED to stage N]` -- moved to a later stage's scope.

Phase A closed 2026-05-06 with all 7 top-level D-tags +
29 sub-decisions LOCKED across a single combined cluster.

---

## 1. Decision discussions

### D-12-1 -- Item schema + ItemKind shape [LOCKED]

**Question.** Stage-1 deferred the full Item schema to
stage 12. Stage-11 D-11-3 + D-11-5 anticipated Item-side
absorption of `fn_sig: FnSigId` and `body: Option<Body>`
under the `Function` ItemKind variant. Stage 12 must lock
the Item struct, the ItemKind cardinality and per-variant
data shape, the Item-level typed-metadata fields anticipated
by stage-1 R5.2 (`linkage: Linkage`, `inline_hint:
InlineHint`, `is_unsafe: bool`, etc.), the accessor pattern
for variant-discriminated data, and the relationship to
stage-14's anticipated `DomainAttr` framework.

**Options considered.**
- (A) Stage-11's framing baked literally: ItemKind 6-variant
  with `Function { fn_sig: FnSigId, body: Option<Body>, ... }`
  struct-variant carrying post-mono per-Function data on Item.
  Non-Function variants stay unit; per-mono data lives on
  InstanceKind::Static.{ty, init, mutability} / Const.{ty,
  value} / {Struct, Enum, Union}.{layout}.
- (B) **Restored symmetry**: ItemKind 6-variant mostly unit;
  per-mono data (fn_sig, body) moves to `InstanceKind::Function
  { fn_sig: FnSigId, body: Option<Body>, flavor, virtual_idx }`.
  Item-level metadata (linkage, inline_hint, is_unsafe, is_naked,
  link_section, lang-item flags, opaque_handle_eligible) lives
  on `Item` directly with sensible defaults for non-fn variants.
  Stage-11 D-11-3 + D-11-5 reopen.

**Decision.** (B) restored-symmetry. Per spec-snapshot.md §8:

```rust
pub struct Item {
    pub def_path: DefPath,
    pub kind:     ItemKind,
    pub span:     Span,

    // Cross-variant Item-level metadata (D-12-1.3)
    pub linkage:        Linkage,
    pub link_section:   Option<StringId>,

    // Function-relevant flags (Item-level with defaults for non-fn variants)
    pub inline_hint:    InlineHint,
    pub is_unsafe:      bool,
    pub is_naked:       bool,

    // Lang-item flags (D-12-3; closed 3-flag set per §8.2)
    pub is_phantom_data:  bool,
    pub is_unsafe_cell:   bool,
    pub is_manually_drop: bool,

    // Bridge-computed outcome (already pinned at §8.0)
    pub opaque_handle_eligible: bool,
}

pub enum ItemKind {
    Function,
    Static { mutability: Mutability },
    Const,
    Struct,
    Enum,
    Union,
}
```

**Reasoning.**
- (A) is incoherent under the rest of the spec's pre-mono
  Item model. §16.1 F4 sort comparator is `(crate_name,
  crate_stable_id, segments)` with no GenericArgs — Items
  are 1:1 with rustc DefIds (pre-mono). §13.4 source-derived
  Items use rustc's DefPathHash verbatim (per-DefId, not
  per-(DefId, GenericArgs)). §9.3 RegseqInstanceKey carries
  `generic_args_key` alongside `item_id` (one Item, multiple
  Instances per generic — implies Item is pre-mono).
- But stage-3 §15.1 rejects `Param/Bound/Placeholder/Infer/
  Alias` TyKind variants. So a stored FnSig must be
  already-substituted (post-mono); Item.kind.fn_sig under
  (A) cannot store the generic shape.
- The two views are incompatible. (B) resolves the
  contradiction: Items are pre-mono (1:1 with rustc DefIds);
  per-mono FnSig + Body live on Instance per the symmetric
  pattern with Static/Const/Struct/Enum/Union.
- Stage-11's "4-peer convergence" citation (rustc,
  rustc_public, Charon, mrustc) was re-examined and found
  to actually support per-Instance body. rustc_public's
  `Instance::body() -> Option<Body>` at `mono.rs:53-55` is
  the cited pattern (per-Instance, NOT per-Item).
- Item-level Function-flags at the Item layer (rather than
  per-variant on `ItemKind::Function`) keeps the schema
  flat: a `static` is never naked or inline; the defaults
  ("off") are well-defined.
- Stage-9 placement of `mutability: Mutability` on
  `InstanceKind::Static` was a denormalization (statics
  can't be generic; 1:1 Item:Instance always; mutability
  is a source-level keyword choice). Moving to
  `ItemKind::Static { mutability }` honors stage-1
  intent (`stage-1/spec-snapshot.md:840-841`).

**Methodology rules invoked.**
- *Compositional behavior beats special-case flags* — per-
  mono data on Instance, source-level alphabet on Item; no
  parallel "is_function_with_body" / "is_static_mutable"
  flags duplicating variant-discriminator info.
- *C-semantics with information preservation* — preserve
  rustc's per-DefId metadata that's load-bearing for C
  codegen (linkage, inline, naked, link_section); other
  CodegenFnAttrs flags (target_features, optimize, ...) fold
  into emission_name or are out-of-v1-scope.
- *Spec is mutable* — stage-11 D-11-3 + D-11-5 reopen
  cleanly under Option B; stage-12 sees the full
  cross-stage picture and can identify the contradiction
  stage-11's local view missed.
- *Re-derive from evidence; do not import baggage* — Item
  schema re-derived from stage-1 R5.2 + stage-3 use-of-
  Item.linkage + stage-4 stage-3-scoped attribute-table
  framing + rustc evidence on per-DefId metadata + 4-peer
  prior-art.

**Prior-art evidence.**
- rustc-internal: 14-variant ItemKind at `rustc_hir/src/
  hir.rs:4397-4464` with first-class Struct/Enum/Union;
  dedicated `ForeignItem` struct + ForeignItemKind at
  `rustc_hir/src/hir.rs:4567-4602`; `has_body: bool` flag
  distinguishes declared-but-unimplemented domestic Fn
  from foreign decls. Per `prior-art/rustc.md`.
- rustc_public: minimal 4-variant `ItemKind { Fn, Static,
  Const, Ctor(CtorKind) }` at `rustc_public/src/lib.rs:120-
  131` with ADT recovered via type system; `is_foreign_item()`
  boolean predicate at `lib.rs:173`; **`Instance::body() ->
  Option<Body>` at `mono.rs:53-55`** is the per-Instance body
  pattern that actually supports Option B (not the per-Item
  framing stage-11 cited). Per `prior-art/rustc_public.md`.
- Charon: orthogonal split `ItemKind { TopLevel, Closure,
  TraitDecl, TraitImpl }` (scope) + `TypeDeclKind { Struct,
  Enum, Union, Opaque, Alias }` (ADT) at `src/ast/gast.rs:
  97-130` + `src/ast/types.rs:460-476`; `Result<Body, Opaque>`
  body encoding at `gast.rs:150` (closest peer to
  RegseqModel's `body: None` for foreign per D-12-2). Per
  `prior-art/charon.md`.
- mrustc: category-based tagged unions (TypeItem / ValueItem
  / MacroItem); Linkage-based foreign encoding; TraitMarkings
  cached trait flags. Per `prior-art/mrustc.md`.

**Worked example.**
```rust
// Source: pub static FOO: u32 = 42;
let foo_item = Item {
    def_path: ...crate::FOO...,
    kind: ItemKind::Static { mutability: Mutability::Not },
    span: src.rs:5:1..5:25,
    linkage: Linkage::Default,
    link_section: None,
    inline_hint: InlineHint::None,        // default for non-fn
    is_unsafe: false,                     // default for non-fn
    is_naked: false,                      // default for non-fn
    is_phantom_data: false,
    is_unsafe_cell: false,
    is_manually_drop: false,
    opaque_handle_eligible: false,
};
// Single Instance for the static (statics can't be generic):
let foo_instance = Instance {
    item: foo_item_id,
    generic_args: vec![],
    emission_name: ...,
    span: src.rs:5:1..5:25,
    kind: InstanceKind::Static { ty: t_u32, init: Some(c_42) },
    // mutability NOT here -- per D-12-1.2 (c) it lives on ItemKind::Static
};

// Source: pub fn add<T: Add>(x: T, y: T) -> T { x + y }
// Generic; one Item, multiple Instances post-mono:
let add_item = Item {
    def_path: ...crate::add...,
    kind: ItemKind::Function,
    linkage: Linkage::Default,
    inline_hint: InlineHint::None,
    is_unsafe: false,
    is_naked: false,
    ...
};
// One Instance per monomorphization (T=u32, T=u64, ...):
let add_u32_instance = Instance {
    item: add_item_id,
    generic_args: vec![GenericArg::Type(t_u32)],
    kind: InstanceKind::Function {
        fn_sig: t_fn_u32_u32_to_u32,         // SUBSTITUTED post-mono
        body:   Some(monomorphized_body_u32), // SUBSTITUTED post-mono
        flavor: FunctionInstanceFlavor::Item,
        virtual_idx: None,
    },
    ...
};
let add_u64_instance = Instance { ..., generic_args: vec![GenericArg::Type(t_u64)], ... };
```

**Spec cross-reference.** §8.0 (Item schema + ItemKind),
§9.4 (InstanceKind::Function + InstanceKind::Static),
§14.3 (FnSig placement reframe under Option B),
§23.6.2 (callee_fn_sig helper updated).

**Sub-decisions.**

#### D-12-1.1 -- Variant cardinality [LOCKED]

**Decision.** Closed 6-variant flat ItemKind:
`{ Function, Static, Const, Struct, Enum, Union }`.

**Reasoning.** rustc_public's 4-variant minimum projection
(without Struct/Enum/Union) is a stable-IR concern (external
tools recover ADT kind via type system); RegseqModel
consumes ADT layout for C codegen and needs first-class
ADT discriminants. rustc-internal's 14-variant set lifts/
erases items RegseqModel rejects (Trait, TraitAlias, Impl,
AssocTy/AssocFn/AssocConst, Macro, ExternCrate, Use,
ForeignMod, Mod, GlobalAsm, TyAlias) per §8.1 lifting policy
and §15.1 type rejections. Charon's orthogonal split
ItemKind/TypeDeclKind adds indirection without semantic value
under Q3 single-level IR. mrustc's category-based tagged
unions force every consumer to dispatch on category before
kind. The 6-variant flat set is the minimum that mirrors
InstanceKind 1:1 (per stage-1 §8.1 framing).

#### D-12-1.2 -- Per-variant ItemKind data shape (Option B + Static.mutability) [LOCKED]

**Decision.** Under Option B, `ItemKind::Function` becomes
unit (per-mono data moves to `InstanceKind::Function`).
`ItemKind::Static { mutability: Mutability }` carries
source-level mutability per stage-1 intent at
`stage-1/spec-snapshot.md:840-841`. Const / Struct / Enum
/ Union stay unit. **Reopens stage-9** (the decision that
placed `mutability` on `InstanceKind::Static`).

**Reasoning.** Statics can't be generic in Rust; 1:1
Item:Instance always. Mutability is a source-level keyword
choice (`static` vs `static mut`); placing it on Item
(source-level alphabet) per stage-1 intent matches Option B's
symmetry argument: source-level metadata on Item, per-mono
data on Instance. (a) status quo (mutability on InstanceKind)
contradicts stage-1 intent; (c) duplicating across both
denormalizes; (b) cleanest. **Cross-stage reopening** to
flag at merger contract close: `InstanceKind::Static` becomes
2-field `{ ty: TyId, init: Option<ConstId> }`.

#### D-12-1.3 -- Item-level typed fields [LOCKED]

**Decision.** Per stage-1 R5.2 + the Option-B simplification
(Function-specific flags become Item-level with sensible
defaults for non-fn variants), the Item-level typed fields:

| Field | Type | Applies-to | Default for non-applicable variants |
|---|---|---|---|
| `linkage` | `Linkage` (closed 4-variant) | Function + Static | n/a (always meaningful) |
| `link_section` | `Option<StringId>` | Function + Static | None |
| `inline_hint` | `InlineHint` (closed 4-variant) | Function | InlineHint::None |
| `is_unsafe` | `bool` | Function (`unsafe fn`) | false |
| `is_naked` | `bool` | Function (`#[naked]`) | false |

**Linkage** closed 4-variant:
```rust
#[repr(u8)]
pub enum Linkage {
    Default,    // ordinary defined fn/static; mangled name
    NoMangle,   // #[no_mangle] / #[export_name]; symbol_name overrides
    Foreign,    // extern "C" { ... } declaration
    Weak,       // #[linkage = "weak"]
}
```

**InlineHint** closed 4-variant:
```rust
#[repr(u8)]
pub enum InlineHint {
    None,    // no #[inline]
    Hint,    // #[inline]
    Always,  // #[inline(always)]
    Never,   // #[inline(never)]
}
```

**Visibility omitted.** Stage-1 R5.2 named it but no v1
driver use case requires it; not load-bearing for one-CU C
codegen. Reopens if phase-2 surfaces analysis-side need.

**Reasoning.** rustc's `CodegenFnAttrs` carries 17+ flags +
many sub-fields (`symbol_name`, `link_name`, `target_features`,
`linkage`, `import_linkage`, `alignment`, `optimize`, etc.).
Most are folded into `Instance.emission_name` per §9.2
(mangling is symbol-emission-time concern; not Item-level
analysis surface) or are out of v1 driver scope (target
features, optimize hints, alignment overrides). Lift only
the load-bearing subset:
- `inline` is observable in C output (`static inline` /
  `__attribute__((always_inline))` / `__attribute__((noinline))`);
  rustc tracks on every fn (cheap). 4-variant closed mirror
  of rustc's `InlineAttr`.
- `is_naked` is structural for ISR entry stubs (kernel-driver
  use case); without it, codegen breaks raw-asm bodies.
- `is_unsafe` is Codex-review-relevant for analyses ("all
  unsafe fn calls must be inside controlled boundaries"); per
  stage-1 R5.2 explicit naming.
- `linkage` is load-bearing per stage-1 R5.2 + stage-3
  references; closed 4-variant `{ Default, NoMangle, Foreign,
  Weak }` covers driver use cases. Other rustc Linkage variants
  (LinkOnceAny, LinkOnceODR, Appending, Common, Internal,
  Private, External-vs-default-external) are LLVM-specific
  and rejected at bridge per D-12-2.3 + §17.4
  closed-vocabulary rule.
- `link_section` is load-bearing for kernel-driver
  auto-discovery (Linux `MODULE_DEVICE_TABLE`, Zephyr
  `device_handles`, `.init_array` / `.driver_data`); full
  string flexibility (closed-vocab not justified — section
  names are kernel/runtime-defined, not RegseqModel-defined).

**Function-flags at Item level (not per-variant on
`ItemKind::Function`).** Cleaner schema; flat match arms;
defaults are well-defined for non-fn variants (a `static` is
never naked or inline). Per "compositional behavior beats
special-case flags" methodology rule.

#### D-12-1.4 -- Option B InstanceKind::Function reframe [LOCKED]

**Decision.** Restore symmetry across all 6 InstanceKind
variants. Per §9.4:

```rust
pub enum InstanceKind {
    Function {
        fn_sig:      FnSigId,                  // post-mono substituted signature
        body:        Option<Body>,             // post-mono substituted body; None for foreign
        flavor:      FunctionInstanceFlavor,   // existing
        virtual_idx: Option<u32>,              // existing
    },
    Static { ty: TyId, init: Option<ConstId> },
    Const  { ty: TyId, value: ConstId },
    Struct { layout: TyId },
    Enum   { layout: TyId },
    Union  { layout: TyId },
}
```

**Reopens stage-11 D-11-3 + D-11-5** — tracked separately;
flips both to `[REOPENED]` in `decisions.md` at stage-12
merger contract close per README §5.5 step 4 with annotation
`(revised in stage 12 per D-12-1.4)`.

**Reasoning.** Stage-11 D-11-3 (FnSig location: Item-side)
and D-11-5 (no BodyId; bodies as `Item.body: Option<Body>`)
were placed on Item but contradicted the rest of the spec's
pre-mono Item model:

- §16.1 F4 sort comparator `(crate_name, crate_stable_id,
  segments)` carries no GenericArgs — Items are 1:1 with
  rustc DefIds (pre-mono).
- §13.4 source-derived Items use rustc's DefPathHash
  verbatim (per-DefId, not per-(DefId, GenericArgs)).
- §9.3 RegseqInstanceKey carries `generic_args_key`
  alongside `item_id` — implies one Item per DefId, multiple
  Instances per generic.
- §3.1 invariant 2 says `item_def_paths[item_id]` is
  rustc's `DefPathHash` for source-derived items —
  per-DefId.
- Stage-3 §15.1 rejects Param/Bound/Placeholder/Infer/Alias
  TyKind variants — so a generic FnSig with Param TyIds
  CAN'T be stored on Item. If `Item.kind.fn_sig` exists,
  it must be already-substituted (post-mono).
- §23.6.2 `callee_fn_sig` helper retrieved FnSig from Item
  with no substitution step — implies post-mono.

These views are incompatible. Option B resolves: Items
pre-mono (1:1 with DefIds), per-mono FnSig + Body on
Instance.

Stage-11's "4-peer convergence" citation was actually
supportive of per-Instance body placement:
- rustc-internal **directly establishes** per-Instance
  body: `tcx.fn_sig(def_id)` returns
  `EarlyBinder<PolyFnSig>` (generic); body fetched per
  Instance via `tcx.instance_mir(InstanceKind)` per
  `rustc_middle/src/ty/mod.rs:1671`.
- rustc_public **directly establishes** per-Instance
  body: `FnDef::fn_sig()` is generic; per-Instance body
  via **`Instance::body() -> Option<Body>`** at
  `rustc_public/src/mir/mono.rs:53-55`, structurally
  separate from `CrateItem::body()`.
- Charon **supports** per-mono body but doesn't directly
  distinguish Item-side vs Instance-side: with
  `--monomorphize`, `FunDecl` IS post-mono —
  `FunDecl.body: Result<Body, Opaque>` is per-FunDecl
  (Charon doesn't carry the same Item/Instance split).
- mrustc is **informational only**: prior-art evidence
  (per `prior-art/mrustc.md`) covers Function structures
  with linkage metadata but did not directly verify
  body-storage placement.

The two directly-establishing peers (rustc-internal +
rustc_public) carry **bodies per-Instance**, supporting
Option B. Stage-11's body-on-Item framing was a
misreading of the rustc_public citation (which actually
establishes `Instance::body()`, not `CrateItem::body()`,
as the per-mono accessor); Option B restores faithfulness
to that evidence.

**Spec edits required (cross-stage):**
- `spec.md` §25.4 (FnSig location): reframe FnSig as
  per-Instance, not Item-side.
- `spec.md` §25.5 (Body-vs-Item linkage): reframe Body as
  per-Instance structural field on InstanceKind::Function.
- `spec.md` §23.6.2 callee_fn_sig helper: drop Item
  indirection step:
  ```text
  callee_fn_sig(callee_id: InstanceId) -> &FnSig :=
      &model.sigs[model.instances[callee_id].kind.as_function().fn_sig]
  ```
- `spec.md` §9.4 InstanceKind: gain `fn_sig` + `body` on
  `Function` variant.
- `spec.md` §3.1 invariant 10: reword as "Body presence
  matches `InstanceKind::Function.body` shape per Option B."

#### D-12-1.5 -- DomainAttr sibling-table forward-pointer [LOCKED]

**Decision.** Stage 12 adds NO schema for `#[rseq::*]`
property attributes. Stage-14 owns the `DomainAttr` sibling-
table architecture per stage-4 spec-snapshot.md:1287-1299.
Stage 12 only updates §8.0 commentary + §3 forward-pointer
to fix the merge artifact that erased stage-4's
"scoped to stage 3" qualifier on the "no general attribute
table" wording.

**Reasoning.** Stage-1 R5.2 anticipated `DomainAttr` on
Item / Instance; stage-4 explicitly scoped the
"no general attribute table" claim to stage-3 only and
committed to stage-14 sibling tables on `RegseqModel` keyed
by `ItemId` / `InstanceId` / `FieldPath` / `Span`. The
merged spec.md §8.0 dropped the stage-3-scoping qualifier
when absorbing stage-4 content (merge artifact). Stage 12
restores the framing: typed Item fields per D-12-1.3 land
at stage 12; structured DSL property attributes
(`#[rseq::*]`) land in stage-14 sibling tables.

**Why no schema on Item.** Stage-4's commitment is
sibling-table architecture, NOT per-Item fields. Pre-staging
a placeholder field on Item would risk locking a shape
stage-14 hasn't designed (e.g., would the field be
`Vec<DomainAttr>`? `HashMap<DomainKey, Value>`?). Stage 14
adds the table when it lands per §17.4 closed-vocabulary
rule.

**Spec edits.** §8.0 commentary replaces "no general
attribute table on Item" with explicit stage-12-vs-stage-14
split. §3 root struct gets a forward-pointer comment for
stage-14's anticipated `domain_attrs_*` sibling tables (no
field added).

#### D-12-1.6 -- Accessor pattern (hybrid + named struct) [LOCKED]

**Decision.** Hybrid: `InstanceKind::as_function() ->
Option<&FunctionInstanceData>` convenience method + match
arms for exhaustive handling. Named `FunctionInstanceData`
struct extracted from struct-variant for ergonomic
destructuring + accessor return type.

```rust
impl InstanceKind {
    pub fn as_function(&self) -> Option<&FunctionInstanceData> { ... }
    pub fn as_function_mut(&mut self) -> Option<&mut FunctionInstanceData> { ... }
}

pub struct FunctionInstanceData {
    pub fn_sig:      FnSigId,
    pub body:        Option<Body>,
    pub flavor:      FunctionInstanceFlavor,
    pub virtual_idx: Option<u32>,
}
```

**Reasoning.** The `§23.6.2 callee_fn_sig` helper assumes a
single-variant query (`inst.kind.as_function().fn_sig`); a
match-arm-only API would force re-issuing the destructure at
each call site. The hybrid pattern matches rustc-internal's
convention (`as_*` accessors throughout `ItemKind` / `TyKind`).
Naming `FunctionInstanceData` (vs `FunctionData` / `FnInstance`)
mirrors the parent's `InstanceKind::Function` variant +
clarifies the input domain.

#### D-12-1.7 -- Stage-11 reopening process [LOCKED]

**Decision.** Stage-11 D-11-3 + D-11-5 flip to `[REOPENED]`
in `decisions.md` at stage-12 merger contract close per
README §5.5 step 4, with annotation `(revised in stage 12
per D-12-1.4)`. Stage-11's `spec-snapshot.md` is FROZEN
(historical artifact); stage-11 content in top-level
`spec.md` IS edited in-place per README §5.9 (live, not
historical; refined content updates in place).

**Reasoning.** Same precedent as stage-11 D-11-3 itself
(which superseded stage-1 inbound's Body-side FnSig
framing). Cross-stage drift detected at stage 12; stage-11
spec content gets minimal in-place edits (§25.4, §25.5,
§23.6.2, §9.4, §3.1 invariant 10) at stage-12 merger.
Stage-11's other decisions (D-11-1 Body shape, D-11-4
LocalId, D-11-6 visitor, D-11-7 hybrid timing, D-11-8
push API, D-11-K rendering deferral) NOT affected.

#### D-12-1.8 -- Instance-side pull-through helpers [LOCKED]

**Decision.** Methods on `RegseqModel` for Item metadata
pull-throughs given `InstanceId`. v1 inventory:

```rust
impl RegseqModel {
    // Identity / source-level traceback
    pub fn instance_def_path(&self, id: InstanceId) -> &DefPath { ... }
    pub fn instance_span(&self, id: InstanceId) -> Span { ... }

    // Source-level metadata fields per D-12-1.3
    pub fn instance_linkage(&self, id: InstanceId) -> Linkage { ... }
    pub fn instance_link_section(&self, id: InstanceId) -> Option<StringId> { ... }
    pub fn instance_inline_hint(&self, id: InstanceId) -> InlineHint { ... }
    pub fn instance_is_unsafe(&self, id: InstanceId) -> bool { ... }
    pub fn instance_is_naked(&self, id: InstanceId) -> bool { ... }

    // Lang-item flags
    pub fn instance_is_phantom_data(&self, id: InstanceId) -> bool { ... }
    pub fn instance_is_unsafe_cell(&self, id: InstanceId) -> bool { ... }
    pub fn instance_is_manually_drop(&self, id: InstanceId) -> bool { ... }

    // Bridge-computed outcome
    pub fn instance_opaque_handle_eligible(&self, id: InstanceId) -> bool { ... }

    // Post-Option-B InstanceKind::Function pull-throughs
    pub fn instance_fn_sig(&self, id: InstanceId) -> Option<&FnSig> { ... }
    pub fn instance_body(&self, id: InstanceId) -> Option<&Body> { ... }
}
```

**Reasoning.** Every Instance has `Instance.item: ItemId`;
Item-level metadata is reachable via simple pull-through.
Methods-on-`RegseqModel` (option a) chosen over methods-on-
`Instance` (option b — clutters call sites with `&model`
parameter) and wrapper-type `InstanceRef<'m>` (option c —
introduces new type; deferable). The wrapper-type design
can be added as a thin layer if call-site ergonomics
demand. v1 simplicity wins: model owns the data; helpers
naturally live with the data. Composes with §23.6.2
`callee_fn_sig` helper pattern (existing convention).

`instance_<field>()` naming convention prefixes input domain;
field names mirror underlying Item field names (no synonyms).

---

### D-12-2 -- Foreign / extern declaration dispatch [LOCKED]

**Question.** Bridge dispatch rule for foreign declarations
(`extern "C" { fn foo(); }` and `extern "C" { static FOO:
u32; }`). Three coupled signals exist post-D-12-1:
`Item.linkage == Linkage::Foreign`, `InstanceKind::Function.
body == None`, `InstanceKind::Static.init == None`. Stage 12
must pin the canonical bridge dispatch rule (which signal is
authoritative) and how rustc's `is_foreign_item` predicate
maps to model-side state.

**Options considered.**
- (a) Body=None / init=None as primary signal; linkage as
  derived. Body is a structural InstanceKind field; linkage
  is computed from rustc's `tcx.is_foreign_item` at bridge
  construction.
- (b) `Linkage::Foreign` as primary signal; Body=None /
  init=None as consequences. Linkage is a source-level
  classifier on Item; consumers query Item.linkage.

**Decision.** Both signals are bridge-populated from the
same source predicate (`tcx.is_foreign_item(did)`); they
are consistent by construction. Consumers can use either:
`Item.linkage == Linkage::Foreign` (Item-level analysis) or
`InstanceKind::Function.body == None` (Instance-level
codegen). The bridge populates per the per-Item mapping
in D-12-2.3.

**Reasoning.** rustc's `is_foreign_item(did)` (parent
`DefKind == ForeignMod` per `rustc_middle/src/hir/mod.rs:
165-168`) is the single-source-of-truth predicate. Both
`Item.linkage = Linkage::Foreign` and `InstanceKind.body /
.init = None` derive from it. Redundancy is acceptable
because the redundant signals serve different consumer
contexts:
- Item-level analyses (e.g., "list all foreign symbols")
  use `Item.linkage == Linkage::Foreign`.
- Instance-level codegen (e.g., "emit declaration vs
  definition") uses `body.is_none() / init.is_none()`.

The methodology rule "compositional behavior beats
special-case flags" doesn't apply here because the signals
serve different semantic levels (source-level alphabet vs
per-mono emission unit); both are useful.

**Methodology rules invoked.**
- *Store the result rustc gives us for free* — both signals
  derive from `is_foreign_item` at bridge construction;
  cheap to populate.
- *C-semantics with information preservation* — preserve
  the distinction between source-level "this is foreign"
  (analysis) and per-mono "no body to emit" (codegen).

**Prior-art evidence.**
- rustc-internal: dedicated `ForeignItem` struct at
  `rustc_hir/src/hir.rs:4567-4602` with `ForeignItemKind {
  Fn, Static, Type }`. RegseqModel rejects this option per
  D-12-1.4 (stage-11's `body: Option<Body>` shape kept; no
  separate ForeignItem variant). Per `prior-art/rustc.md`.
- rustc_public: boolean `is_foreign_item()` predicate at
  `rustc_public/src/lib.rs:173`. Closest peer to RegseqModel
  D-12-2's combined signal pattern. Per
  `prior-art/rustc_public.md`.
- Charon: `Result<Body, Opaque>` body encoding at
  `gast.rs:150` (comment: "Opaque functions are: external
  functions, or local functions tagged as opaque"). Closest
  peer to RegseqModel's `body: Option<Body>` pattern. Per
  `prior-art/charon.md`.
- mrustc: Linkage-based; ABI on Function struct; no
  dedicated foreign type. Per `prior-art/mrustc.md`.

**Spec cross-reference.** §9.4 (InstanceKind dispatch),
§8.0 (Item.linkage), §3.4 (bridge construction-pipeline).

**Sub-decisions.**

#### D-12-2.1 -- Foreign Function dispatch [LOCKED]

**Decision.** `InstanceKind::Function.body: None ⟺ tcx.
is_foreign_item(callee_did)`. When `is_foreign_item` returns
true:
- `body: None` — rustc's `is_mir_available(did)` is `false`
  per `rustc_monomorphize/src/collector.rs:1059-1062`.
- `fn_sig: FnSigId` is **populated** — `tcx.fn_sig(did)` is
  callable for foreign fns; signature required for indirect-
  call shape and C-prototype emission.
- `flavor: FunctionInstanceFlavor::Item` (foreign decls are
  not shims).
- `virtual_idx: None`.

#### D-12-2.2 -- Foreign Static dispatch [LOCKED]

**Decision.** `InstanceKind::Static.init: None ⟺ tcx.
is_foreign_item(static_did)`. When `is_foreign_item` returns
true:
- `init: None` — rustc has no const-eval result for foreign
  statics; value provided at link time.
- `ty: TyId` is **populated** — `tcx.type_of(did)` is callable.
- Mutability lives on `ItemKind::Static { mutability }` per
  D-12-1.2 (c); InstanceKind::Static is now 2-field
  `{ ty, init }`.

#### D-12-2.3 -- Linkage translation table [LOCKED]

**Decision.** Bridge translation map from rustc to RegseqModel
`Linkage`:

| Source signal | RegseqModel `Linkage` |
|---|---|
| `tcx.is_foreign_item(did) == true` | `Linkage::Foreign` |
| `CodegenFnAttrFlags::NO_MANGLE` set OR `#[export_name]` (`symbol_name.is_some()`) | `Linkage::NoMangle` |
| `CodegenFnAttrs.linkage == Some(WeakAny | WeakODR)` | `Linkage::Weak` |
| Otherwise (default external + mangled) | `Linkage::Default` |

Other rustc `Linkage` variants (LinkOnceAny, LinkOnceODR,
Appending, Common, Internal, Private, External-when-distinct-
from-default) are rejected at the bridge with
`BridgeError::UnsupportedLinkage`. Driver fixtures don't
use these; rejecting keeps closed-vocabulary discipline
intact.

#### D-12-2.4 -- link_name placement [LOCKED]

**Decision.** `link_name` is NOT on Item per D-12-1.3
(omitted). The mangled / overridden symbol lives on
`Instance.emission_name: StringId` per §9.2 (rustc's
`tcx.symbol_name(MonoItem::*)` mangled name). Bridge populates
`Instance.emission_name` from rustc's symbol-name query,
which already incorporates `#[link_name]` overrides for
foreign items.

**Reasoning.** Mangling is symbol-emission-time concern,
not Item-level analysis surface. Folding into
`Instance.emission_name` consolidates: one place for
the mangled name; consumers read `instance.emission_name`
without needing a parallel `Item.link_name` lookup.

---

### D-12-3 -- Lang-item recognition mechanism [LOCKED]

**Question.** Stage-1 + §8.2 currently pin 3 lang-item flags
on Item (`is_phantom_data`, `is_unsafe_cell`,
`is_manually_drop`). Stage 12 must lock the recognition
mechanism formally and document the closed-vocabulary
discipline for non-flagged lang-items (CVoid, MaybeUninit,
Pin, Box, Drop trait, drop_in_place, Fn/FnMut/FnOnce traits,
panic_fn, etc.). Specifically: where is the recognition done
(bridge construction time vs at use site), and is there an
opaque catch-all (Charon-style `lang_item: Option<String>`)
for lang-items not in the closed flag set?

**Options considered.**
- (a) Closed 3-flag set per current §8.2; AdtFlags-predicate-
  based recognition; non-flagged lang-items queried on-demand
  at consumer sites; reject opaque LangItem tag.
- (b) Expand to 5+ flag set including Pin, Box, etc.; or
  include MaybeUninit explicitly.
- (c) Catch-all `lang_item: Option<LangItem>` opaque tag
  (rustc has 208 LangItem variants per Fork 2 evidence) or
  `lang_item: Option<String>` (Charon-style) on every Item.

**Decision.** (a) closed 3-flag set + AdtFlags predicates +
query-on-demand for non-flagged + reject opaque.

**Reasoning.**
- Each currently-flagged lang-item is load-bearing for
  type-system or codegen lowering: PhantomData triggers ZST
  handling; UnsafeCell triggers §17.2.1 trait-object dispatch
  carve-out; ManuallyDrop triggers no-auto-drop union
  semantics. AdtFlags-predicate recognition is rustc's
  canonical fast path (O(1) bitflag tests) per
  `rustc_middle/src/ty/adt.rs:30-63, 414-440`.
- Non-flagged lang-items (CVoid, Pin, Box, etc.) are consumed
  at specific bridge call sites (type translation, drop-as-
  call collapse, etc.) and never need Item-walk-time access.
  Storing them on Item would denormalize and force every
  Item-walk to consider an irrelevant flag.
- `Pin` deferred: reintroduce `is_pin` when async support
  lands per current §8.2 commitment.
- `Box`, `CVoid`: rejected at bridge per stage-3 D1.4 / D16;
  no flag needed (rejection happens at type translation).
- `MaybeUninit`: composition (Union no-auto-drop + inner
  ManuallyDrop) suffices per stage-3 D1.5 reasoning.
- Opaque LangItem tag (208-variant rustc enum) violates
  §17.4 closed-vocabulary rule. Adding new lang-item
  recognition is a methodology event (per current §8.2
  "Reopens when a v1 driver fixture or stage-14 intrinsic
  surfaces a stable need"); opaque tag would let
  toolchain-bump-introduced variants slip in without that
  gate.
- Charon-style `lang_item: Option<String>` admits silent
  typo / drift; Charon's choice is justified by Charon's
  source-of-truth-is-rustc design; RegseqModel is a
  closed-vocabulary IR per §17.4 + Q3 single-level IR.

**Methodology rules invoked.**
- *Closed `Ty` enum* (§17.4 generalized to all closed
  vocabularies in RegseqModel) — opaque tag rejected.
- *Compositional behavior beats special-case flags* —
  MaybeUninit composes from existing Union + ManuallyDrop;
  no parallel flag.
- *Store the result rustc gives us for free, interrogated
  per construct* — only store load-bearing-for-codegen
  outcomes; query the rest on-demand.
- *No defensive fallbacks* — toolchain-bump LangItem
  expansions reject explicitly; no silent admit-as-string
  fallback.

**Prior-art evidence.**
- rustc-internal: 208-variant `LangItem` enum at
  `rustc_hir/src/lang_items.rs:79-451`; `tcx.is_lang_item(did,
  LangItem::X)` query pattern; `tcx.lang_items().X()` accessor
  pattern; AdtFlags bits for PhantomData/UnsafeCell/
  ManuallyDrop/Pin/Box at `rustc_middle/src/ty/adt.rs:30-63`.
  Per `prior-art/rustc.md`.
- rustc_public: NO lang-item exposure in stable API. Per
  `prior-art/rustc_public.md`.
- Charon: string-based `lang_item: Option<String>` at
  `src/ast/meta.rs:231` — RegseqModel rejects this shape.
  Per `prior-art/charon.md`.
- mrustc: TraitMarkings cached trait flags
  (`has_a_deref`, `has_drop_impl`, `is_copy`, ...). Per
  `prior-art/mrustc.md`.

**Spec cross-reference.** §8.2 (Item-level lang-item flags),
§17.4 (closed-vocabulary rule).

**Sub-decisions.**

#### D-12-3.1 -- Closed 3-flag set carries [LOCKED]

**Decision.** No expansion in v1. Carries forward §8.2's
3-flag set unchanged: `is_phantom_data`, `is_unsafe_cell`,
`is_manually_drop`. `is_pin` deferred per current §8.2;
`is_c_void` / `is_box` rejected (recognition at type
translation; bridge rejects).

#### D-12-3.2 -- Recognition via AdtFlags predicates [LOCKED]

**Decision.** Bridge populates the 3 flags via rustc's
predicate methods at type translation time:
```text
when bridge translates an Adt to an Item:
    item.is_phantom_data  = adt_def.is_phantom_data();   // AdtFlags::IS_PHANTOM_DATA
    item.is_unsafe_cell   = adt_def.is_unsafe_cell();    // AdtFlags::IS_UNSAFE_CELL
    item.is_manually_drop = adt_def.is_manually_drop();  // AdtFlags::IS_MANUALLY_DROP
```

#### D-12-3.3 -- Non-flagged lang-items queried on-demand [LOCKED]

**Decision.** Lang items NOT in the §8.2 closed flag set are
recognized at the consumer site via `tcx.is_lang_item(did,
LangItem::X)` or `tcx.lang_items().X()` accessor — never
stored on `Item`.

#### D-12-3.4 -- Reject opaque LangItem tag and string-based field [LOCKED]

**Decision.** No `Option<LangItem>` opaque tag; no
Charon-style `lang_item: Option<String>` field on Item.
Closed-vocabulary discipline per §17.4. Adding new
lang-item recognition is a methodology event (Codex review
of the new flag's semantic placement); opaque tag would
defeat the gate.

---

### D-12-4 -- Construction-invariant closure (stage-1 deferrals 3, 4, 6) [LOCKED]

**Question.** Stage 1 deferred three construction invariants
to stage 12 pending stage-12 schema commits:
- **Invariant 3** (F4 sort monotonicity): items are placed
  in F4-sorted order; validate must check.
- **Invariant 4** (BridgeInput spec_crate_stable_id half):
  `crates[primary_crate].stable_crate_id ==
  bridge_input.spec_crate_stable_id`.
- **Invariant 6** (def_path.crate_id bound):
  `item.def_path.crate_id.index() < crates.len()`.

Stage 12 must specify the algorithms, landing site within
the §3.3 Phase B validation order, and (for invariant 4) the
`BridgeInput` schema + validate signature change.

**Options considered.**
- (a) Lift all three to a new validation phase. Too coarse;
  duplicates structure §3.3 Phase B already establishes.
- (b) Land within §3.3 Phase B step 3 (Items / crates /
  target / build / invariants). All three checks operate on
  Items + crates which are already validated in this phase.
  Cohesive landing.

**Decision.** (b) within §3.3 Phase B step 3, reordered to
(a)..(g) per the new sub-step ordering below.

**Reasoning.**
- All three checks operate on `items` and `crates` —
  already-validated tables at Phase B step 3.
- Ordering matters: invariant 6 (crate_id bound) before
  invariant 3 (F4 monotonicity walk) because the F4 sort
  key reads `crates[item.def_path.crate_id].name` — bound
  check must precede sort-key derivation.
- Invariant 4 depends only on `crates` (already validated)
  and `primary_crate` (already model-checkable per stage-1);
  can land anywhere in the block.
- BridgeInput parameter on validate is a controlled
  signature change; v1 schema is single-field
  `{ spec_crate_stable_id }` with open-extension for future
  bridge-supplied facts.

**Methodology rules invoked.**
- *Spec is mutable* — invariant ordering refined to capture
  the bound-before-walk dependency.
- *No defensive fallbacks* — invariants are model-checkable
  from RegseqModel + BridgeInput; no rustc-side re-derivation
  needed.

**Prior-art evidence.** N/A — these are stage-1-deferred
mechanical invariants; algorithms are bound checks +
monotonicity walks. Direct primary-source for the
walk-discipline shape: rustc's MIR validator at
`rustc_mir_transform/src/validate.rs` uses a similar
walk-and-check pattern.

**Spec cross-reference.** §3.1 invariants 3, 4, 6;
§3.3 Phase B step 3 (revised); §16.1 (F4 sort comparator);
new `BridgeInput` struct.

**Sub-decisions.**

#### D-12-4.1 -- Invariant 3 closure (F4 sort monotonicity) [LOCKED]

**Decision.** Walk `items` index-pair-wise; for each adjacent
pair `(items[i], items[i+1])`, compute the F4 sort key per
§16.1 and check **non-strict** monotonicity (`key[i] <=
key[i+1]`). Out-of-order is `ValidationError::F4OutOfOrder
{ item_id, prev_key, next_key }`.

```text
fn validate_f4_monotonicity(model: &RegseqModel) -> Result<(), ValidationError> {
    for i in 0..model.items.len().saturating_sub(1) {
        let key_i   = f4_sort_key(model, ItemId::from(i));
        let key_ipp = f4_sort_key(model, ItemId::from(i + 1));
        if key_i > key_ipp {
            return Err(ValidationError::F4OutOfOrder { ... });
        }
    }
    Ok(())
}
```

**Tie semantics.** Adjacent items with identical sort keys
are NOT a violation — F4 admits ties on identical DefPath.
Check is `>` (strict descent), not `>=`.

**Tests.** `tests/model.rs`: positive (3-item F4-sorted) +
negative (3-item reverse-F4) cases.

**Landing site.** Phase B step 3 (d).

#### D-12-4.2 -- Invariant 6 closure (def_path.crate_id bound) [LOCKED]

**Decision.** Walk `items`; for each item, check
`item.def_path.crate_id.index() < model.crates.len()`.
Out-of-bounds is `ValidationError::DefPathCrateIdOutOfBounds
{ item_id, crate_id, crates_len }`.

**Landing site.** Phase B step 3 (c) — before invariant 3
walk per the bound-before-key-derivation dependency.

#### D-12-4.3 -- Invariant 4 closure (BridgeInput.spec_crate_stable_id) [LOCKED]

**Decision.** Add `BridgeInput` struct to `regseq-model`
(zero rustc deps; pure data type per Q1):

```rust
pub struct BridgeInput {
    pub spec_crate_stable_id: StableCrateId,
}
```

Validate signature changes from `RegseqModel::validate(&self)
-> Result<...>` to `RegseqModel::validate(&self,
bridge_input: &BridgeInput) -> Result<...>`.

Algorithm:
```text
fn validate_spec_crate_stable_id(
    model: &RegseqModel,
    bridge_input: &BridgeInput,
) -> Result<(), ValidationError> {
    let primary = &model.crates[model.primary_crate];
    if primary.stable_crate_id != bridge_input.spec_crate_stable_id {
        return Err(ValidationError::SpecCrateStableIdMismatch { ... });
    }
    Ok(())
}
```

**Test ergonomic helper.** `BridgeInput` exposes
`BridgeInput::matching(model: &RegseqModel) -> Self` for
test fixtures that don't care about invariant 4
specifically (constructs a `BridgeInput` with
`spec_crate_stable_id` matching the model's
`crates[primary_crate].stable_crate_id`).

**Landing site.** Phase B step 3 (e).

**Phase B step 3 final order:**
```text
(a) Validate `crates`: stable_crate_id uniqueness + sorted (existing — invariant 5).
(b) Validate `items` shape: length parity (existing — invariant 2 length parity).
(c) Validate `item.def_path.crate_id < crates.len()` for every item (D-12-4.2).
(d) Validate F4 sort monotonicity over `items` (D-12-4.1).
(e) Validate `crates[primary_crate].stable_crate_id == bridge_input.spec_crate_stable_id` (D-12-4.3).
(f) Validate `target` well-formedness (existing — invariant 15).
(g) Validate `build` well-formedness (existing — invariant 17).
```

---

### D-12-5 -- Bridge construction-pipeline order [LOCKED]

**Question.** Stages 1, 3, 4, 9, 11 each framed pieces of
the bridge construction pipeline (rustc mono-item collector
discovery; vtable / closure / ConstMemoryStatic synthesis
per §13; F4 sort; ItemId assignment; side-table population;
Instance construction). Stage 12 must consolidate these
pieces into a single canonical pipeline contract: the
ordering, where synthesis interleaves, and when ItemIds are
assigned. The model-side observable property (post-sort
F4-deterministic ItemIds) is what consumers depend on; the
contract must commit to this shape without over-specifying
bridge-implementation details.

**Options considered.**
- (a) Implementation-detail delegation: pipeline is bridge's
  business; spec only commits to externally-observable
  invariants (F4-sorted Items; valid cross-references; etc.).
- (b) **Canonical 5-phase pipeline contract**: spec pins the
  ordering Discovery -> Translation+Synthesis -> F4 Sort ->
  ItemId Assignment + side-tables -> Instance Construction.
  Synthesis interleaves with translation. ItemIds post-sort.
  Cross-reference handling during construction is bridge
  choice (placeholder vs DefPathHash key).

**Decision.** (b) canonical 5-phase pipeline contract; new
§3.4 subsection.

**Reasoning.**
- (a) is too loose. Stage-3 D5/D6 + stage-4 D-4-3 + stage-9
  + stage-11 inbound bundles cite synthesis triggers (e.g.,
  `Cast::Unsize` triggers vtable instance Item registration)
  that need a contract on when synthesis happens. Without
  the contract, bridge implementations could diverge.
- (b) catalogs the pipeline that already implicitly exists
  across stage spec contributions; gives bridge implementers
  a single landing site for the contract.
- The 5-phase shape is the natural decomposition: Discovery
  is a one-shot rustc query; Translation+Synthesis is the
  interleaved walk; F4 Sort is a one-shot pass over the
  accumulated set; ItemId Assignment is the post-sort
  permutation; Instance Construction is the per-mono walk
  that resolves cross-references.
- Pin externally-observable property (post-sort
  F4-deterministic ItemIds); leave cross-reference handling
  during construction (placeholder ItemIds vs DefPathHash
  key) as bridge-implementation choice — it's not externally
  observable.

**Methodology rules invoked.**
- *Compositional behavior beats special-case flags* —
  pipeline order falls out of Q2 (eager mono baking) + the
  F4 comparator + the synthesis-trigger rules in §13; no
  parallel pipeline-stage tags.
- *C-semantics with information preservation* — pipeline
  contract preserves rustc's mono-item closure (Phase 1)
  and F4 deterministic ordering (Phase 3) as the two
  primary external commitments.

**Prior-art evidence.**
- rustc-internal: `rustc_monomorphize/src/collector.rs:1803`
  (`collect_crate_mono_items` entry); `rustc_codegen_ssa/
  src/base.rs:681` (`codegen_crate` outer loop). Per
  rustc-references.md.
- rustc_public: `MonoItem` set + per-Instance `body()`
  fetch pattern (no separate sort phase; rustc_public
  doesn't commit to Item ordering).
- Charon: monomorphize mode runs after extraction; phase-
  ordering is similar but Charon doesn't make the F4-
  determinism commitment.
- mrustc: standard collector + monomorphize flow.

**Spec cross-reference.** New §3.4 (canonical 5-phase
pipeline); §13 (synthesis triggers); §16.1 (F4 sort
comparator).

**Sub-decisions.**

#### D-12-5.1 -- Canonical 5-phase pipeline structure [LOCKED]

**Decision.** Pinned 5-phase pipeline:

```text
Phase 1  -- DISCOVERY.
    rustc's `collect_crate_mono_items(tcx, strategy)` returns
    Vec<MonoItem> covering primary crate + regseq-aware deps
    per Q6.

Phase 2  -- TRANSLATION (with interleaved Item synthesis).
    Bridge walks discovered mono-items. During translation,
    synthesis triggers fire and produce additional Items per §13:
      - Ty::Dynamic       -> VTableType Item per §13.2 (D5)
      - Cast::Unsize      -> VTableInstance + VTableShim Items per §13.2 (D5)
      - Ty::Closure       -> ClosureCaptures Item per §13.1 (D6)
      - closure call site -> ClosureCall Function Items per §13.1 (D6)
      - non-trivial drop  -> ClosureDropGlue Function Item per §13.1 (D6)
      - reference-typed const -> ConstMemoryStatic Item per §13.3 (D-4-3)
      - Drop terminator   -> drop-glue Function Item synthesis
        (the call target for the Phase-5 Drop-as-Call rewrite)
        per stage-9 §23.4
    Note: the Drop-as-Call collapse itself (rewriting Drop into
    Statement+Call inside a Body) is a Phase-5 body-construction
    step, not a Phase-2 Item-synthesis trigger; Phase 2 only
    synthesises the drop-glue Function Item.

Phase 3  -- F4 SORT.
    Sort accumulated Items (source + synthetic) per §16.1
    comparator: (crate_name, crate_stable_id, segments).

Phase 4  -- ItemId ASSIGNMENT + side-table population.
    Sorted Items get sequential ItemIds: ItemId(0..items.len()).
    `item_def_paths[ItemId]` and `item_synthetic_kinds[ItemId]`
    populated in parallel.

Phase 5  -- INSTANCE construction + cross-reference resolution
    + body-content rewrites + validation. `instances` per §9.2
    (one Instance per (Item, GenericArgs, rustc shim flavor)
    tuple). Sorted by F4-2 RegseqInstanceKey per §16.3.
    Cross-references resolved to final ItemIds.
    `RegseqModel.vtables` dedup table populated. Body-content
    rewrites sequence: (a) Drop-as-Call collapse (per §25.7.1;
    rewrite Drop terminator into Statement+Call referencing
    the Phase-2-synthesised drop-glue Function Item), then
    (b) vtable-indirect rewrite (per §25.7.2; intra-Phase-5
    pass over materialised Bodies), then (c) joint validation
    walk per §25.6.
```

#### D-12-5.2 -- Synthesis interleaving [LOCKED]

**Decision.** Bridge-synthesised Items are produced **during**
Phase 2 translation, NOT in a separate pre-pass that runs
before discovery or after.

**Reasoning.** Synthesis triggers are construct-driven
(`Ty::Dynamic` encountered → vtable type Item); the bridge
can't pre-enumerate triggers without first translating.
Synthesis must complete before Phase 3 F4 sort (sort needs
the full Item set).

**Cache discipline.** Each synthesis trigger checks a
**family-specific Phase-2 working cache** keyed by a stable
Phase-2-available handle (NOT final `ItemId`, which doesn't
exist until Phase 4 per D-12-5.3). Examples:
- Vtable family: `PrincipalTraitKey -> Phase2Handle`.
- Closure family: `closure_did -> Phase2Handle` (rustc's
  closure DefPathHash is stable and Phase-2-available).
- ConstMemoryStatic family: `ContentHash -> Phase2Handle`.

Cache hit returns the existing Phase2Handle; cache miss
registers a new synthetic Item shape (with synthesised
DefPathHash per §13.4 as the natural Phase-2 stable key).
Idempotent: revisiting the same trigger during Phase 2
yields the previously-registered Phase2Handle.

The model-side `RegseqModel.vtables: HashMap<PrincipalTraitKey,
ItemId>` (and analogous final-form caches) is populated in
**Phase 5** (after final ItemIds exist via the Phase-4
assignment pass per D-12-5.3). Phase 2 working caches use
Phase2Handles; Phase 5 walks them and re-keys to final
ItemIds.

#### D-12-5.3 -- Post-sort ItemId assignment [LOCKED]

**Decision.** Final `ItemId` values are assigned at Phase 4
(after F4 sort), not incrementally during Phase 2. F4 +
build-equivalence quad (§12.4) commit to deterministic
ItemIds across builds with the same source. Incremental
assignment would tie ItemIds to rustc's collector traversal
order, which can vary with incremental compilation cache
state.

**Cross-reference handling during construction.** Bridge
implementation choice; not externally observable. Two viable
patterns:
- **(a) Placeholder ItemIds + remap.** Bridge assigns
  provisional registration-order indices during Phase 2;
  computes the F4 sort permutation at Phase 3; remaps all
  cross-references to final ItemIds at Phase 4 / 5.
- **(b) DefPathHash as canonical key.** Bridge uses
  DefPathHash for cross-references during Phase 2; final
  ItemIds assigned at Phase 4; cross-references resolved
  via the `DefPathHash → ItemId` map after sort.

Both produce identical model-side state. Spec only commits
to **post-sort F4-deterministic ItemIds** as the externally-
observable property.

#### D-12-5.4 -- Spec landing site (new §3.4) [LOCKED]

**Decision.** New subsection §3.4 "Bridge construction-
pipeline order" lands at stage-12 merger; sibling to §3.1
invariants, §3.2 Local IDs, §3.3 validation. Catalogs the
5-phase contract; cross-refs §13 (synthesis), §16.1 (F4
sort), §3.1 invariants 2 / 3 / 9 (item_def_paths /
F4-monotonicity / instance closure-completeness).

---

### D-12-6 -- `render_def_path(ItemId)` stub fill [LOCKED]

**Question.** Stage-1 ships a `render_def_path` rendering
helper and an eager `Item.def_path.rendered: StringId`
cache. Stage-1 stub at `crates/regseq-model/src/pretty.rs`
panics with "stage 12" message. Stage 12 must specify the
fill body.

**Options considered.**
- (a) Re-render at print time using `render_def_path_value
  (&DefPath)`. Forces the canonical-rendering rule to run
  every print.
- (b) **Cache lookup**: read `Item.def_path.rendered`
  directly and resolve via `model.strings`. One-line body;
  trivial.

**Decision.** (b) cache lookup. One-line body:

```rust
pub fn render_def_path(&self, id: ItemId) -> String {
    self.model.strings.resolve(self.model.items[id].def_path.rendered).to_owned()
}
```

**Reasoning.**
- The eager `Item.def_path.rendered` field is the cached
  output of `render_def_path_value(&DefPath)` (the value-form
  helper shipped by stage 1, used at bridge construction
  time).
- §3.3 Phase B "DefPath.rendered matches canonical rendering"
  check (existing) re-derives the canonical rendered form
  from `crate_id + segments` and verifies equality with
  `rendered` — so the cached form is authoritative and
  validation-checked.
- (a) would do redundant work (re-derive on every print)
  for no benefit.

**Methodology rules invoked.**
- *Store the result rustc gives us for free* — eager
  rendering is computed once at construction, cached, and
  re-used. Pretty-printer trusts the cache.

**Prior-art evidence.** N/A — stub-fill-with-cache-lookup is
standard. Stage-3's `render_ty` follows similar pattern at
stage-13 (per D14 deferral).

**Spec cross-reference.** §7.2 (DefPath.rendered eager
canonical rendering); §18.2 per-method ownership table
(updates per D-12-6.3).

**Sub-decisions.**

#### D-12-6.1 -- One-line body [LOCKED]

**Decision.** Body as written above. No re-rendering at print
time; pure cache resolution.

#### D-12-6.2 -- Coexistence with `render_def_path_value(&DefPath)` [LOCKED]

**Decision.** Both helpers coexist with documented division
of labor:
- `render_def_path(ItemId)` — ItemId-indexed convenience
  accessor; short-circuits via the cached `rendered` field.
- `render_def_path_value(&DefPath)` — value-level renderer;
  used for paths NOT attached to an Item (e.g.,
  `PrincipalTraitKey.trait_path` is a DefPathHash for an
  erased Trait def with no Item; or diagnostic dumps of
  synthetic DefPaths during bridge Phase 2 before ItemIds
  are assigned).

Both call the same canonical rendering rule (§7.2 verbose
form); the ItemId form short-circuits via cache.

#### D-12-6.3 -- §18.2 per-method ownership table update [LOCKED]

**Decision.** §18.2 row for `render_def_path` flips from
"deferred to stage 12" to "stage 12; D-12-6". Stage-1's
`render_def_path_value(&DefPath)` row remains unchanged.

---

### D-12-K -- Categorical Item-level rendering deferral to stage 13 [LOCKED]

**Question.** Stage-13 owns body-level rendering per
stage-11 D-11-K (the cascade absorbing D-5-K through D-10-K
+ stage-1 `_N` sigil + stage-10 `bbN` sigil). Stage 12
introduces Item-level metadata (linkage, inline_hint,
is_unsafe, is_naked, link_section, lang-item flags,
opaque_handle_eligible) that needs rendering. Plus the
ItemKind variant rendering, full Item display rendering,
and FunctionData (now FunctionInstanceData under Option B)
pretty-printing. Where does the rendering for these
Item-level constructs land?

**Options considered.**
- (a) Stage 12 owns Item-level rendering. But stage 12 is
  the last spec stage before stage 13 (pretty-printing),
  and stage 13 already owns the rendering style decisions
  for body-level content per D-11-K. Splitting Item-level
  vs body-level rendering across two stages introduces
  inconsistent style.
- (b) **Categorical deferral compositional with D-11-K**:
  all Item-level rendering except `render_def_path`
  (D-12-6) defers to stage 13. Stage 13 owns Item-level +
  body-level rendering as a single coherent rendering-
  decision surface.

**Decision.** (b) categorical deferral.

**Reasoning.**
- Item-level metadata renders coherently with body-level
  content. A Function Item's display couples its source-
  level metadata (linkage, inline_hint) to its rendered
  body (via the Option-B `InstanceKind::Function { fn_sig,
  body }` lookup). Stage 13 already owns body-level
  rendering per D-11-K; coupling Item-level rendering to
  the same stage keeps the rendering surface a single
  coherent design.
- `render_def_path` is special: its body is a trivial
  cache lookup that doesn't make a "rendering style"
  decision. The canonical form is locked at stage 1's
  `render_def_path_value`; stage 12 just retrieves it.
- D-11-K rationale (stage 13 picks rendering style, sigil
  conventions, per-method ownership coherently across
  body-level constructs) extends naturally to Item-level
  renderers.

**Methodology rules invoked.**
- *Spec is mutable* — D-12-K extends D-11-K's cascade
  cleanly; no contradiction with existing rendering
  decisions.
- *Compositional behavior beats special-case flags* —
  Item-level rendering doesn't introduce a parallel "is
  Item display" flag; existing per-method ownership table
  (§18.2) carries the deferral attribution.

**Prior-art evidence.** N/A — the deferral-cascade pattern
is RegseqModel-internal, established by stage-11 D-11-K.

**Spec cross-reference.** §18.2 per-method ownership table
(updates per D-12-K.3); stage-11 D-11-K (cascade
precedent).

**Sub-decisions.**

#### D-12-K.1 -- Deferral list [LOCKED]

**Decision.** The following Item-level rendering items
defer to stage 13:

1. `render_item(ItemId)` (full-Item display renderer).
2. ItemKind variant rendering (per-variant formatting;
   includes `Static { mutability }` per D-12-1.2).
3. `Linkage` enum rendering (4-variant closed enum).
4. `InlineHint` enum rendering (4-variant closed enum).
5. Function-flag rendering (`is_unsafe`, `is_naked`).
6. `link_section: Option<StringId>` rendering decorator.
7. Lang-item flag rendering (3 flags from §8.2).
8. `opaque_handle_eligible` flag rendering decorator.

`render_def_path(ItemId)` is the only non-deferred
Item-level renderer (stays at stage 12 per D-12-6).

#### D-12-K.2 -- Outbound bundle to file at merger close [LOCKED]

**Decision.** At stage-12 merger contract close (per
README §5.5 step 7), file an outbound expectation bundle
in `stage-13/expectations.md` "From stage 12 (per D-12-K)"
with the deferral list above. No predecessor K-tag
retargeting (D-12-K is purely additive — D-11-K already
covered all body-level rendering; D-12-K covers Item-level
rendering, which D-11-K didn't address).

#### D-12-K.3 -- §18.2 per-method ownership table updates [LOCKED]

**Decision.** §18.2 gets new rows for the 7-entry deferral
bundle, each marked "deferred to stage 13 per D-12-K".
Existing `render_def_path` row updates to "stage 12;
D-12-6" per D-12-6.3.

---

## 2. Phase A discussion threads (substantive)

Three substantive Phase A threads warrant standalone
recording for Phase B audit-shape capture (in addition to
their per-D-tag entries above).

### Option B reframe of stage-11 D-11-3 + D-11-5 (D-12-1.4)

The load-bearing finding of stage-12 Phase A. Mid-cluster
discussion surfaced the asymmetry between Function and
non-Function ItemKind variants: stages had committed to
per-mono data on InstanceKind for Static/Const/Struct/Enum/
Union, but stage-11 placed `fn_sig` and `body` on Item under
the `Function` ItemKind variant.

The contradiction surfaced: the rest of the spec
(F4 sort comparator + §13.4 source DefPathHash + §9.3
RegseqInstanceKey) carries Item as pre-mono (1:1 with
rustc DefIds), but stage-3 §15.1 rejects Param/Bound
TyKind variants — so a stored FnSig must be already-
substituted (post-mono). The two views are incompatible.

Stage-11's "4-peer convergence" citation
(rustc, rustc_public, Charon, mrustc) was re-examined and
found to actually support per-Instance body, NOT per-Item
body:
- rustc-internal: `tcx.fn_sig` is generic; body fetched
  per-Instance via `tcx.instance_mir(InstanceKind)`.
- rustc_public: `FnDef::fn_sig()` is generic; **`Instance::
  body() -> Option<Body>`** at `rustc_public/src/mir/mono.rs:53-55`
  (per-Instance) is the actually-cited pattern, structurally
  separate from `CrateItem::body()`.
- Charon: `FunDecl` is post-mono under `--monomorphize`
  (supports per-mono body but doesn't directly distinguish
  Item-side vs Instance-side).
- mrustc: informational only (prior-art evidence didn't
  directly verify body-storage placement).

The two directly-establishing peers (rustc-internal +
rustc_public) carry bodies per-Instance. RegseqModel's
stage-11 framing of body-on-Item was a misreading of the
rustc_public citation (the cited 4-peer pattern was
`Instance::body()`, not `CrateItem::body()`); Option B
restores faithfulness to that evidence.

**Resolution:** lock Option B at D-12-1.4. Reopens stage-11
D-11-3 + D-11-5 (tracked separately; flips to `[REOPENED]`
in `decisions.md` at stage-12 merger close per D-12-1.7).
Stage-11 spec content requires minimal in-place edits
(~50-100 lines of in-place updates to §25.4, §25.5,
§23.6.2, §9.4, §3.1 invariant 10). Stage-11's other
decisions (D-11-1 Body shape, D-11-4 LocalId, D-11-6
visitor, D-11-7 hybrid timing, D-11-8 push API, D-11-K
rendering deferral) are NOT affected.

### Static.mutability placement reopens stage-9 (D-12-1.2)

Stage-1 `spec-snapshot.md:840-841` explicitly placed
mutability on `ItemKind::Static { mutability, .. }`; the
current merged spec.md §9.4 has it on
`InstanceKind::Static`. Investigation revealed the move to
InstanceKind happened during stage-9 closure without
re-litigating stage-1's intent.

Option B's symmetry argument extends here: source-level
metadata on Item; per-mono data on Instance. Mutability is
a source-level keyword choice (`static` vs `static mut`);
statics can't be generic in Rust → 1:1 Item:Instance always
→ no per-mono variation possible.

**Resolution:** lock D-12-1.2 (c). Move mutability to
`ItemKind::Static { mutability: Mutability }`; remove from
`InstanceKind::Static` (now 2-field `{ ty, init }`).
**Reopens a stage-9 decision** — specific D-tag identifying
at stage-12 merger contract close.

### Stage-3-scoped attribute-table wording (D-12-1.5)

Stage-4 `spec-snapshot.md:1287-1299` explicitly scoped the
"no general attribute table on Item" wording to **stage 3
only**, AND committed to stage-14 `DomainAttr` sibling
tables on `RegseqModel` keyed by `ItemId` / `InstanceId` /
`FieldPath` / `Span`. The merged spec.md §8.0 dropped the
"scoped to stage 3" qualifier when absorbing stage-4
content (merge artifact).

**Resolution:** D-12-1.5 restores the framing in §8.0
commentary. Stage 12 adds typed Item fields per D-12-1.3;
stage 14 adds structured DSL property attributes
(`#[rseq::*]`) as RegseqModel sibling tables. No stage-12
schema for DomainAttr.

---

## 3. Cross-references

- `../README.md` -- top-level project mission, methodology
  rules, architectural decisions Q1-Q7.
- `../decisions.md` -- registry of all per-stage D-tags;
  stage-12 entries appended at merger contract close.
- `../discussions.md` -- merged audit trail; stage-12
  contributions absorbed at merger contract close.
- `../spec.md` -- merged top-level spec; stage-12
  contributions land per README §5.9 update guidelines.
- `expectations.md` -- inbound forward refs from stages 1,
  3, 9, 11; stage 4 already CLOSED by D-4-3.
- `memory.md` -- stage-12 invariants (workflow, principles,
  read order, Codex prompt template).
- `status.md` -- stage-12 current-state tracker (locked
  decisions, active focus, next concrete unit of work).
- `prior-art/{rustc,rustc_public,charon,mrustc}.md` --
  per-stage primary-source research framing the D-tags.
- `spec-snapshot.md` -- stage-12 working spec with the
  Phase B contributions.
- `review-log.md` -- stage-12 round-by-round Codex audit
  trail (lazy; created at first review round).

Cross-stage references (load-bearing for Phase B):
- `../stage-1/spec-snapshot.md:82-89` -- stage-1 Rust→C
  lowering reference table; `Linkage`, `InlineHint`,
  `is_unsafe`, `DomainAttr` rows pinned to stage 12 / 14.
- `../stage-1/spec-snapshot.md:820-824` -- stage-1
  `instance.item`'s attributes / ABI / visibility on Item;
  R5.2 binding commitment.
- `../stage-1/review-log.md:391-407` -- R5.2 resolution
  pinning stage-12 typed-field commitment.
- `../stage-3/spec-snapshot.md:884-887, 949, 1022` --
  stage-3 use of `Item.linkage` as load-bearing for
  symbol-mangling decisions.
- `../stage-4/spec-snapshot.md:1279-1299` -- stage-4
  stage-3-scoped attribute-table commentary + stage-14
  forward expectation for `DomainAttr` sibling tables.
- `../stage-11/spec-snapshot.md:§25.4 + §25.5` -- stage-11
  D-11-3 + D-11-5 framing (FnSig + Body on Item; reopened
  by D-12-1.4 Option B).
- `../stage-14/expectations.md:152-157` -- stage-14
  `DomainAttr` parsing commitment.

---

## 4. Outbound expectations to file at merger contract close

Per README §5.5 step 7 + D-12-K.2 / D-12-1.7, the
following outbound expectations are filed at stage-12
merger contract close:

### To `stage-13/expectations.md` "From stage 12 (per D-12-K)"

Categorical Item-level rendering deferral bundle (per
D-12-K.1):

```text
1. render_item(ItemId) -- full-Item display renderer.
2. ItemKind variant rendering -- per-variant formatting hooks.
3. Linkage enum rendering -- 4-variant {Default, NoMangle, Foreign, Weak}.
4. InlineHint enum rendering -- 4-variant {None, Hint, Always, Never}.
5. Function-flag rendering (is_unsafe, is_naked) + lang-item flag rendering
   (is_phantom_data, is_unsafe_cell, is_manually_drop).
6. link_section: Option<StringId> rendering decorator.
7. opaque_handle_eligible flag rendering decorator.
```

Predecessor-stage attribution: stage 12 (Items).

### To `stage-14/expectations.md` "From stage 12"

Carry over stage-1 + stage-4 forward expectations
(unchanged; stage 14 owns the property vocabulary):

```text
1. Q7 intrinsic recognition mechanism builds on
   stage-12's lang-item-recognition pattern (closed-
   vocabulary + tcx.intrinsic_raw(did) + per-callee
   #[rseq::intrinsic("NAME")] attribute lookup at call
   translation). Stage-12's 3-flag set is the template;
   stage-14 generalises.
2. DomainAttr sibling tables on RegseqModel root keyed by
   ItemId / InstanceId / FieldPath / Span per stage-4
   §1287-1299. Stage-12 confirmed the architecture (no
   per-Item field; sibling-table-only). Stage-14 picks
   the per-property keying.
```

### Cross-stage reopenings flagged at merger close

Per D-12-1.7 + D-12-1.2 (c) sub-decision findings:

- **Stage-11 D-11-3** (FnSig location: Item-side) -> flip
  to `[REOPENED]` in `decisions.md` with annotation
  `(revised in stage 12 per D-12-1.4)`. FnSig moves to
  `InstanceKind::Function`.
- **Stage-11 D-11-5** (no BodyId; bodies as `Item.body`)
  -> flip to `[REOPENED]` similarly. Bodies move to
  `InstanceKind::Function.body: Option<Body>`.
- **Stage-9 InstanceKind::Static.mutability placement** ->
  flip to `[REOPENED]` per D-12-1.2 (c). Mutability moves
  to `ItemKind::Static`. Specific stage-9 D-tag identified
  at merger contract close via `decisions.md` review.

---

## 5. F-12 cross-stage findings (informational)

Findings surfaced during stage-12 evidence pass that are
NOT stage-12 D-tags but worth recording for cross-stage
coherence. Action items at merger close.

### F-12-Reify -- ReifyShim payload drift (stage-9 territory)

Rustc's `ty::InstanceKind::ReifyShim` payload has expanded
since stage-9 was written: now `(DefId, Option<ReifyReason>)`
distinguishing vtable-vs-fn-pointer reification contexts
(per `rustc_middle/src/ty/instance.rs:40-60` documentation
block). This may break stage-9's `RegseqInstanceKey`
injectivity invariant: two `ReifyShim` instances with
different `ReifyReason` would share `(rank, did, args)` and
collapse.

**Stage-12 disposition:** stage 12 is Item-level work; this
is an Instance-level concern (stage-9 territory). No
stage-12 schema change.

**Action at merger close:** flag in stage-12 outbound
findings for stage-9's next review cycle. If codegen / phase-2
device pressure surfaces a real `ReifyReason` collision (two
distinct shims sharing `(did, args)`), stage 9 reopens to
extend `FunctionInstanceFlavor::ReifyShim` with the
`ReifyReason` discriminator or extend `RegseqInstanceKey`
similarly. Otherwise document in `../status.md` "Deferred
items (cross-stage)" and leave to future stage-9 review.

**Cross-refs:**
- `rustc_middle/src/ty/instance.rs:40-185`.
- spec.md §9.3 (RegseqInstanceKey).
- spec.md §9.4 (FunctionInstanceFlavor::ReifyShim).
