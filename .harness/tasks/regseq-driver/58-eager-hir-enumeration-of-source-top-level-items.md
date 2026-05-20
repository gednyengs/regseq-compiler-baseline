# regseq-driver-58: Eager HIR enumeration of source-defined top-level items

## Description

Close the Item-enumeration asymmetry surfaced by manual playground
stress-testing against a fixture defining `struct Reg(u32)` + `impl Reg
{ const REG_0; const REG_1; fn new(off) -> Reg }` + `pub fn entry()`:
the rendered `model.txt` contains `entry`, `REG_0`, `REG_1` -- but
NOT `Reg` itself and NOT `Reg::new`. Both reviewers (manual + Codex)
flagged SIGNIFICANT.

Root cause: regseq-driver-55 back-filled Phase-5 Instance construction
for source ADTs/statics/consts/synth VTableType, but it implemented
enumeration asymmetrically:

- **Consts**: enumerated EAGERLY via `tcx.hir_crate_items()` filtered
  to `DefKind::Const | AssocConst` at
  `crates/regseq-driver/src/bridge/mod.rs:717-738`. Every top-level
  const surfaces regardless of reachability.
- **Statics**: enumerated EAGERLY via the same HIR walk at the same
  callsite.
- **Structs/Enums/Unions**: enumerated REACTIVELY -- only types
  referenced by some translated `Ty::Adt(_)` get interned as Items
  (`crates/regseq-driver/src/bridge/ty.rs:286-324`). The Phase-5
  pass `construct_nominal_adt_instances`
  (`crates/regseq-driver/src/bridge/mod.rs:1559-1601`) then walks
  already-interned nominal `Ty` rows; unused non-generic ADTs have
  zero Ty rows, zero Items, zero Instances.
- **Functions**: enumerated REACTIVELY -- only fns reached by rustc's
  `MonoItem::Fn` collection at
  `crates/regseq-driver/src/bridge/mod.rs:657-703`. Private uncalled
  fns are pruned by the mono closure.

The spec language anchoring the fix is regseq-driver-55's Description:
"Every source-defined `struct` / `enum` / `union` / `static` / `const`
at the source crate's top-level surfaces as a corresponding
`Instance`." That contract was not realized for ADTs or functions.

After a five-question spec dialogue (Q1-Q5, closed and confirmed) the
contract is now:

1. **Item population**: HIR-driven eager walk. Every HIR-discoverable
   source/foreign top-level def lands in `model.items` -- regardless
   of visibility, generics, or reachability. Covers ItemKind variants
   `Function`, `Static`, `Const`, `Struct`, `Enum`, `Union` per
   spec.md §8.0 (`spec.md:2205-2230`) and the impl-lifting rules at
   §8.1 (`spec.md:2334-2358`): inherent impl methods, trait impl
   methods, default trait method bodies (Codex adjudication (b)),
   and associated consts all lift to top-level Items. Trait
   declarations and associated types are intentionally erased per
   `spec.md:2346-2350`.

2. **Instance population**: monomorphization-driven, four buckets:
   - **Bucket 1** -- non-generic + signature-rigid + non-foreign:
     exactly 1 Instance per Item, via trivial identity-substitution
     monomorphization. The bridge eagerly computes layout (for ADTs)
     or signature + body (for fns) or type + init (for statics/consts).
   - **Bucket 2** -- foreign (`tcx.is_foreign_item(did) &&
     !tcx.is_mir_available(did)`): exactly 1 Instance with
     `body: None` (for fns) or `init: None` (for statics) per
     spec.md §9.4 Function body at `spec.md:2642-2648` and Static
     init at `spec.md:2668-2677`.
   - **Bucket 3** -- generic: 0 or more Instances per Item; reactive
     via `Ty::Adt(_)` translation or `MonoItem::Fn` walk (as today).
     Generic Items with zero monomorphizations have zero Instances.
   - **Bucket 4** -- non-generic + signature-non-rigid (RPIT case
     like `pub fn closure_fixture() -> impl Fn(u32) -> u32` at
     `crates/regseq-driver/tests/bridge_synth.rs:399`): 0 Instances.
     Documented carve-out keyed on `translate_fn_sig` rejection
     (`crates/regseq-driver/src/bridge/fnsig.rs`); replaces the
     regseq-driver-55-v3 blanket Function carve-out.

3. **Emission**: pretty-printer + backends iterate `model.instances`
   only. Item-only entries (bucket 3 unused, bucket 4) stay
   invisible. This rule is UNCHANGED from today; the pretty-printer's
   "does NOT iterate `model.items`" guarantee at
   `pretty-printer.md:4249` continues to hold.

4. **Validator (`RegseqModel::validate_bridge_completeness`)**: Item
   coverage + Instance coverage per the four buckets. Retires both
   blanket skips at `crates/regseq-model/src/model.rs:3475-3486`
   (foreign-skip and Function-skip); replaces with bucket-keyed
   enforcement.

Sub-questions closed during the dialogue:

- **(a) Foreign-static rendering**: ALREADY SPECIFIED at
  `pretty-printer.md:3640-3698`; Specimen 120 at
  `pretty-printer.md:2236-2244` shows
  `#[foreign]\nstatic libc::stderr: *mut libc::FILE;`. No
  pretty-printer.md amendment needed.

- **(b) Trait default method bodies**: AMBIGUOUS in spec; Codex
  recommended position: default methods with bodies surface as
  `ItemKind::Function` Items at the trait's DefPath. This rides
  with the task as a small spec.md §8.1 clarification (Change H).

After this task lands:

- The playground stress fixture `struct Reg(u32) + impl Reg { const
  REG_0; const REG_1; fn new(off) -> Reg } + pub fn entry()` renders
  with `Reg`, `Reg::new`, `REG_0`, `REG_1`, and `entry` all present.
- Foreign decls (`extern "C" { fn printf(); static errno: i32; }`)
  surface as Function/Static Instances with `body: None` / `init:
  None`, rendered per Specimen 120 form.
- A new e2e fixture exercises every bucket on a single crate.
- The validator enforces the four-bucket policy via three new
  helper predicates on `Item` (or model side-tables -- implementer's
  choice).

## Input Context

### Spec anchors

- `docs/discussions/regseq-model/spec.md` §8.0 ItemKind alphabet
  (`spec.md:2125-2230`) -- the six-variant closed alphabet
  Function/Static/Const/Struct/Enum/Union.
- `docs/discussions/regseq-model/spec.md` §8.1 impl-lifting rules
  (`spec.md:2334-2362`) -- inherent and trait impl methods lift;
  associated consts lift; trait declarations and associated types
  are erased.
- `docs/discussions/regseq-model/spec.md` §8.1 default-trait-method
  text (`spec.md:2351-2352`) -- "Default trait method bodies are
  owned by the trait, not by any impl. The body lives at the trait's
  DefPath." (Ambiguous on Item shape; Change H clarifies.)
- `docs/discussions/regseq-model/spec.md` §9.4 InstanceKind variants
  -- §9.4 header `spec.md:2618`; Function variant `spec.md:2618-2648`
  with `body: None` carve-out for foreign decls; Static variant
  `spec.md:2668-2677` with `init: None` for foreign decls;
  layout-bearing Struct/Enum/Union variants further into §9.4.
- `docs/discussions/regseq-model/pretty-printer.md` Static rendering
  (`pretty-printer.md:3640-3698`) -- the
  `static [mut ]<path>: <ty>[ = <init>];` shape supports
  `init: None` as semicolon-terminated declaration.
- `docs/discussions/regseq-model/pretty-printer.md` Specimen 120
  (`pretty-printer.md:2236-2244`) -- foreign static rendering with
  `#[foreign]` attribute.
- `docs/discussions/regseq-model/pretty-printer.md` Instance
  iteration guarantee (`pretty-printer.md:4203-4249`) -- the
  pretty-printer iterates `model.instances` and "does NOT iterate
  `model.items`"; Item-only entries remain invisible.

### Bridge code anchors

- `crates/regseq-driver/src/bridge/mod.rs:657-703` -- current
  Phase-2 `MonoItem::Fn` walker; pre-interns reachable Function
  Items only.
- `crates/regseq-driver/src/bridge/mod.rs:717-738` -- current
  eager HIR walk for `DefKind::Const | AssocConst`. The model for
  Change A / B / C; needs extending to cover `Fn | AssocFn |
  Static | ForeignFn | ForeignStatic | Struct | Enum | Union`.
- `crates/regseq-driver/src/bridge/mod.rs:1236-1324`
  (`construct_function_instances`) -- the regseq-driver-55 Phase-5
  pass that creates Function Instances for `ItemKind::Function`
  Items. Currently fires only for Items the MonoItem walk
  pre-interned; Change E extends it to all non-generic + sig-rigid
  + non-foreign Function Items including newly HIR-discovered ones.
- `crates/regseq-driver/src/bridge/mod.rs:1379-...` --
  `construct_source_static_instances`; per Codex review it
  currently excludes foreign statics (`mod.rs:1379`). Change F
  removes the foreign exclusion and adds the `init: None` path.
- `crates/regseq-driver/src/bridge/mod.rs:1559-1601` --
  `construct_nominal_adt_instances`; walks already-interned nominal
  `Ty` rows. Change D extends it to fire on every non-generic ADT
  Item, triggering identity-substitution `Ty` / layout interning
  when no row exists yet.
- `crates/regseq-driver/src/bridge/mod.rs:1978-2019`
  (`canonicalize_call_site_callee`) -- the regseq-driver-56 helper;
  unchanged.
- `crates/regseq-driver/src/bridge/mod.rs:2049-...`
  (`source_item_def_id`) -- scans `tcx.hir_body_owners()`. Codex
  flagged this won't cover foreign decls (no body owners); Change B
  must use a different resolver for `DefKind::ForeignFn` /
  `DefKind::ForeignStatic` (likely
  `tcx.hir_crate_items().foreign_items()` or similar). `extern type`
  opaque types are out of scope.
- `crates/regseq-driver/src/bridge/ty.rs:286-324`
  (`translate_adt`) -- the reactive ADT Item interner. Stays
  functional; Change D's eager pass dedupes against it.
- `crates/regseq-driver/src/bridge/fnsig.rs::translate_fn_sig` --
  the predicate that distinguishes signature-rigid from
  signature-non-rigid. `translate_fn_sig` rejection produces
  `NonRigidTy` errors. Change E uses this rejection as the bucket-4
  carve-out signal.

### Model code anchors

- `crates/regseq-model/src/model.rs::validate_bridge_completeness`
  (`model.rs:3442-3486`) -- the validator with the
  regseq-driver-55-v3 blanket Function carve-out + foreign skip.
  Change G rewrites it.
- `crates/regseq-model/src/model.rs::Item` -- the struct that
  carries `linkage: Linkage`, `is_phantom_data`, etc. Change G
  likely adds `is_generic: bool` and `signature_non_rigid: bool`
  flags (implementer may pick a side-table instead).

### Prior tasks + carve-outs preserved

- regseq-driver-55 (closed at `9dc619d`): Phase-5 Instance
  construction back-fill for source ADTs / statics / consts / synth
  VTableType. Spec language anchor for this task.
- regseq-driver-55-v3 (`6cddf5d`): narrowed the Function carve-out
  in `validate_bridge_completeness` to retire RPIT-uncalled-fn
  failure. Change G subsumes the carve-out into bucket 4.
- regseq-driver-56 (closed at `f6d30a5`): call-site canonicalization
  for static-dispatch trait methods. Unchanged.
- regseq-driver-57 (closed at `2a93b34`): diag_log recognizer
  cleanup. Unchanged.

### Playground reproduction case (manual verification artifact)

- `tests/fixtures/playground/src/lib.rs` -- the user's stress-test
  fixture. Not modified by this task.
- `tests/fixtures/playground/regseq-out/model.txt` --
  before-fix evidence at lines 1-11 (asymmetric enumeration: `entry`
  + `REG_0` + `REG_1` present, `Reg` + `Reg::new` absent). Change J
  records this evidence in a new file
  `.harness/expectations/regseq-driver-58.md`. Note: that file is
  an implementation deliverable produced by the implementer
  alongside the code change, NOT a pre-review input.

## Implementation Notes

### Change A: eager HIR walk for ADT Items

**Anchor**: `crates/regseq-driver/src/bridge/mod.rs:717-738`.

Extend the existing eager HIR walk that handles `Const | AssocConst`
to also handle `DefKind::Struct | Enum | Union`. For each
HIR-discovered ADT DefId:

1. Compute `is_generic = tcx.generics_of(def_id).count() > 0`.
2. Compute `linkage = Linkage::Default` (ADTs are never foreign).
3. Call the existing reactive entry point
   (`translate_adt`-equivalent path) with identity-substitution args
   (`ty::GenericArgs::identity_for_item(tcx, def_id)`) to intern the
   Item via the established interner. For generic ADTs, the identity
   substitution may itself be insufficient to compute layout -- in
   that case, intern the Item shell only (no `Ty` row, no Instance);
   Change D handles the Item-only generic case.
4. Record `is_generic` on the Item (new flag or side-table).

This change MUST run before Phase-5 construction passes so the
Phase-5 passes see complete Item coverage.

### Change B: eager HIR walk for foreign decls

**Anchor**: `crates/regseq-driver/src/bridge/mod.rs:717-738` +
`mod.rs:2049-...` (new resolver path).

Iterate `tcx.hir_crate_items().foreign_items()` (or equivalent
HIR helper -- implementer confirms the exact rustc API at impl
time). For each foreign DefId:

1. Compute `linkage = Linkage::Foreign`.
2. Intern the Item via a linkage-aware path. Note: the current
   `intern_item` at `crates/regseq-driver/src/bridge/ty.rs:454-463`
   hardcodes `Linkage::Default`. This task adds a linkage-aware
   variant (either extend `intern_item` to accept a `Linkage`
   parameter, or add a sibling `intern_foreign_item` helper --
   implementer's choice). Foreign items MUST be interned with
   `Linkage::Foreign` so the bucket-2 validator predicate and the
   `#[foreign]` attribute rendering both fire.
3. For `DefKind::ForeignFn` -> `ItemKind::Function`; for
   `DefKind::ForeignStatic` -> `ItemKind::Static { mutability }`
   (mutability via `tcx.static_mutability(def_id)` or the rustc
   HIR foreign-item kind).
4. DO NOT use `source_item_def_id` (`mod.rs:2049`) -- it scans
   body-owners which foreign decls lack (Codex new issue #1). Use
   the HIR foreign-items iterator directly.
5. If `translate_ty` (for the foreign static's type) or
   `translate_fn_sig` (for the foreign fn's signature) returns
   `Err(_)`, propagate as `BridgeError`. Do NOT silently skip the
   foreign Item; the toolchain is pinned and a translation failure
   here is a real bridge-input invariant violation per the
   no-defensive-fallback rule.

### Change C: eager HIR walk extension to Function-shape items

**Anchor**: `crates/regseq-driver/src/bridge/mod.rs:717-738`.

Extend the eager HIR walk to also handle `DefKind::Fn | AssocFn`
for source-defined functions. The walk covers:

- `DefKind::Fn` -- free functions at the crate root or in nested
  modules.
- `DefKind::AssocFn` -- inherent and trait impl methods (per spec
  §8.1 impl-lifting at `spec.md:2338-2344`).
- `DefKind::AssocFn` with `tcx.impl_of_assoc(def_id).is_none() &&
  tcx.trait_of_assoc(def_id).is_some()` -- default trait method
  bodies (Change H amendment makes this normative). Only iterate
  these when `tcx.is_mir_available(def_id)` returns true. The
  `is_mir_available` predicate distinguishes trait methods with
  inline default bodies (MIR available, surface as Item) from trait
  method declarations without bodies (MIR unavailable, do NOT
  surface as Item -- consistent with the regseq-driver-56 trait-decl
  erasure rule). Same predicate that spec §9.4 uses to distinguish
  foreign decls from source-defined fns
  (`tcx.is_foreign_item(did) && !tcx.is_mir_available(did)`).

`DefKind::Closure` is OUT OF SCOPE for the eager HIR walk. The
existing closure-call synth path (`bridge/synth/closure.rs`)
handles closures interned reactively via `MonoItem::Fn` walks
(closures are always reached via their enclosing function). The
eager walk MUST NOT preempt that path.

For each surviving DefKind:

1. Compute `is_generic = tcx.generics_of(def_id).count() > 0`.
2. Compute `linkage = Linkage::Default`.
3. Intern the Item via the existing Function-Item interner. The
   Item's `def_path` follows §8.1 lifting (inherent: `crate::T::{impl#N}::method`;
   trait impl: `crate::T::{impl#N as Trait}::method`; trait default:
   `crate::Trait::method`).
4. Record `is_generic` on the Item.

This change MUST run AFTER the existing Phase-2 `MonoItem::Fn` walk
or BEFORE it with dedup. Implementer's choice; the canonical
approach is: run eager HIR walk FIRST, then have the MonoItem walk
dedupe against existing Items via DefPathHash lookup
(`bridge_ctxt.function_instance_for_item` table -- already keyed
on `(ItemId, ArgsKey, FunctionInstanceFlavor)` per
regseq-driver-42-4 + regseq-driver-46).

### Change D: Phase-5 ADT Instance construction extends to bucket-1 unused ADTs

**Anchor**: `crates/regseq-driver/src/bridge/mod.rs:1559-1601`
(`construct_nominal_adt_instances`).

Today this pass walks `model.items[ItemKind::{Struct|Enum|Union}]`
filtering to Items that have at least one nominal `Ty` row in
`model.types`. Extend the pass to ALSO fire on bucket-1 Items (non-
generic + non-foreign + no existing Ty row) by:

1. For each non-generic non-foreign ADT Item with no
   `Ty::Adt(item, _)` row, trigger eager `Ty` interning via
   `translate_ty(ctxt, tcx.type_of(def_id).instantiate_identity())`.
   This produces a `Ty::Struct { item, args: [], layout }` /
   `Ty::Enum { ... }` / `Ty::Union { ... }` row. If `translate_ty`
   returns `Err(_)`, propagate as `BridgeError` -- do NOT silently
   skip. The toolchain is pinned; a translation failure on a
   non-generic non-foreign ADT's identity-substituted type is a
   real bridge-input invariant violation per the
   no-defensive-fallback rule.
2. Push the corresponding `InstanceKind::Struct { layout } /
   Enum { layout } / Union { layout }` Instance keyed on the
   identity-arg Ty.
3. Generic ADTs with no existing Ty rows: skip (bucket 3 with
   zero monomorphizations -> 0 Instances). Skipping here is an
   explicit bucket-3 branch matched by
   `tcx.generics_of(def_id).count() > 0`, NOT a translation-error
   fallback.

### Change E: Phase-5 Function Instance construction extends to bucket-1 unused fns

**Anchor**: `crates/regseq-driver/src/bridge/mod.rs:1236-1324`
(`construct_function_instances`).

Today this pass iterates `model.items[ItemKind::Function]` for Items
the MonoItem walk pre-interned. Extend the pass to ALSO fire on
HIR-walk-discovered Function Items by:

1. For each non-generic non-foreign Function Item without an
   existing Function Instance, try:
   - Call `ty::Instance::try_resolve(tcx, ty::TypingEnv::fully_monomorphized(),
     def_id, identity_args)`. On `Ok(Some(_))`, proceed. On
     `Ok(None)` or `Err(_)`: return `BridgeError::Phase5(format!(
     "non-generic non-foreign fn {def_id:?} failed Instance::try_resolve"))`
     -- do NOT silently `continue` (no-defensive-fallback rule;
     this branch should not fire under the pinned toolchain for a
     non-generic non-foreign fn). Replaces the current silent
     `continue` at `mod.rs:1295-1304`.
   - Call `let fn_sig = translate_fn_sig(ctxt, tcx.fn_sig(def_id)
     .instantiate_identity())`.
   - If `Ok(fn_sig)`: push `InstanceKind::Function {
     fn_sig, body: None, flavor: Item, virtual_idx: None }`.
     Body materialization happens later via the existing body
     pass (`materialise.rs:313-334`).
   - If `Err(BridgeError::NonRigidTy { .. })`: this is bucket 4.
     Record the non-rigid signal on the Item (new flag
     `signature_non_rigid: true` -- or side-table); push NO
     Instance. The validator's bucket-4 carve-out keys on this
     flag.
   - If `Err(e)` for any other `BridgeError` variant: propagate
     the error -- do NOT silently `continue`. The current silent
     `continue` at `mod.rs:1316-1318` becomes:
     `Err(e) if !matches!(e, BridgeError::NonRigidTy { .. }) => return Err(e)`.
2. Generic Function Items: skip (bucket 3 with zero
   monomorphizations -> 0 Instances). Skipping here is NOT a
   defensive fallback -- it's an explicit bucket-3 branch matched
   by `tcx.generics_of(def_id).count() > 0` before any translation
   attempt.
3. Foreign Function Items: handled by Change F.

### Change F: foreign-fn + foreign-static Instance construction

**Anchor**: extends `construct_function_instances`
(`mod.rs:1236-1324`) and `construct_source_static_instances`
(`mod.rs:1379-...`).

For each Foreign Item (Function or Static):

- Function: push `InstanceKind::Function { fn_sig, body: None,
  flavor: Item, virtual_idx: None }` where `fn_sig` comes from
  `tcx.fn_sig(def_id).instantiate_identity()` translated via
  `translate_fn_sig`. Per spec §9.4 `body: None` is reserved for
  foreign decls. If `translate_fn_sig` returns `Err(_)`, propagate
  -- do NOT silently skip the foreign Item. Foreign fn signatures
  must be rigid (no RPIT etc.) since they're C ABI-bound; a
  translation failure is a real input invariant violation per
  the no-defensive-fallback rule.
- Static: push `InstanceKind::Static { ty, init: None }` where
  `ty` comes from `tcx.type_of(def_id).instantiate_identity()`
  translated via `translate_ty`. Per spec §9.4 `init: None` is
  reserved for foreign statics. If `translate_ty` returns
  `Err(_)`, propagate -- do NOT silently skip.

Remove the foreign-skip at `mod.rs:1379` in
`construct_source_static_instances`.

### Change G: validator rewrite -- four-bucket policy

**Anchor**: `crates/regseq-model/src/model.rs:3442-3486`
(`validate_bridge_completeness`).

Replace the current blanket Function-skip (`model.rs:3479-3486`) and
foreign-skip (`model.rs:3475-3478`) with the four-bucket policy:

```rust
for (item_id, item) in model.items.iter() {
    if item.synthetic_kind != SyntheticItemKind::None {
        continue;  // synthetic Items handled by other passes
    }
    let bucket = classify_bucket(item);  // see below
    match bucket {
        Bucket::OneRequired => {
            // Non-generic + sig-rigid + non-foreign
            // -> exactly 1 Instance required
            assert_exactly_one_instance(model, item_id)?;
        }
        Bucket::Foreign => {
            // Foreign -> exactly 1 Instance with body: None / init: None
            assert_exactly_one_instance(model, item_id)?;
            assert_foreign_body_or_init_is_none(model, item_id)?;
        }
        Bucket::GenericMayBeZero => {
            // 0+ Instances allowed
        }
        Bucket::SignatureNonRigid => {
            // Bucket 4 (RPIT) -- 0 Instances allowed
        }
    }
}

fn classify_bucket(item: &Item) -> Bucket {
    if item.linkage == Linkage::Foreign {
        return Bucket::Foreign;
    }
    if item.is_generic {
        return Bucket::GenericMayBeZero;
    }
    if matches!(item.kind, ItemKind::Function) && item.signature_non_rigid {
        return Bucket::SignatureNonRigid;
    }
    Bucket::OneRequired
}
```

Add the two new flags to `Item`:

- `is_generic: bool` -- set by the eager HIR walk (Change A / B / C)
  based on `tcx.generics_of(def_id).count() > 0`.
- `signature_non_rigid: bool` -- set by Change E when the trivial
  identity-substitution `translate_fn_sig` rejects. For non-Function
  Items, always `false`.

Alternative (implementer's choice): store the flags in a side-table
on `RegseqModel` (e.g., `model.bridge_metadata.item_generic_flags:
HashMap<ItemId, BridgeItemFlags>`) to avoid schema churn. The
validator logic is identical either way.

### Change H: spec.md §8.1 clarification for default trait method bodies

**Anchor**: `docs/discussions/regseq-model/spec.md:2351-2352`.

Today reads:

> Default trait method bodies are owned by the trait, not by any
> impl. The body lives at the trait's DefPath.

After Change H:

> Default trait method bodies are owned by the trait, not by any
> impl. The body lives at the trait's DefPath, and surfaces as a
> `ItemKind::Function` Item at the trait's DefPath. The trait
> container itself remains erased (no `ItemKind::Trait` variant).
> Trait methods WITHOUT default bodies (declarations only) do NOT
> surface as Items -- they exist solely as DefPath segments
> referenced by impl-method def_paths per the impl-lifting rules
> above.

Rationale per the Codex (b) adjudication: the body is source-owned
and has a DefPath; not surfacing it would lose source content that
backends might want.

### Change I: integration test fixture + e2e snapshot

**Anchor**: new files
`crates/test-ir-fixtures/fixtures/eager_enumeration.rs` (fixture)
and `crates/regseq-model/tests/e2e_eager_enumeration.rs` (e2e
driver-side test).

Fixture mirrors the bucket-spanning shape:

```rust
#![no_std]

// Bucket 1: non-generic + sig-rigid + non-foreign
pub struct Reg(pub u32);
impl Reg {
    pub const REG_0: u32 = 0;
    pub fn new(off: u32) -> Reg { Reg(off) }
}

// Bucket 3: generic (no monomorphizations used)
pub struct Slot<T>(pub T);

// Bucket 4: non-generic + signature-non-rigid (RPIT)
pub fn make_counter() -> impl Fn(u32) -> u32 {
    |x| x + 1
}

// Bucket 2: foreign
extern "C" {
    pub fn extern_helper(x: u32) -> u32;
    pub static EXTERN_VALUE: u32;
}

// Public entry to keep the crate non-empty and ensure
// MonoItem walk runs.
pub fn entry() {}
```

Snapshot test (`e2e_eager_enumeration.rs`) bridges the fixture
end-to-end and asserts the rendered model contains:

- `struct ...::Reg { 0: u32 }` (bucket 1).
- `const ...::{impl#0}::REG_0: u32 = 0_u32;` (bucket 1).
- `fn ...::{impl#0}::new(_1: u32) -> ::Reg { ... }` (bucket 1).
- `fn ...::entry() -> () { ... }` (bucket 1).
- `#[foreign]\nfn ...::extern_helper(_1: u32) -> u32;` (bucket 2,
  body: None per Specimen 120 form).
- `#[foreign]\nstatic ...::EXTERN_VALUE: u32;` (bucket 2, init: None).
- No render for `Slot<T>` (bucket 3, 0 monomorphizations).
- No render for `make_counter` (bucket 4, 0 Instances). The
  function's CLOSURE may render via the existing synth path
  (the closure `|x| x + 1` is reached if `make_counter` is itself
  reached -- but `make_counter` has bucket-4 status and is never
  called, so the closure stays unreached too).

### Change J: harness expectations entry

**Anchor**: `.harness/expectations/regseq-driver-58.md` (new file).

Record:

- Before-fix evidence from the playground fixture: cite the
  asymmetry where `Reg` + `Reg::new` are absent while
  `Reg::REG_0` + `Reg::REG_1` surface.
- After-fix expected enumeration: list the exact slot enumeration
  expected from the playground fixture once the task lands.
- Normative regression guard: cite the e2e fixture + snapshot from
  Change I as the in-tree coverage that survives any future
  regression.
- Out-of-scope (rides regseq-driver-58 Non-Goals): emission rules,
  pretty-printer shape changes, new ItemKind variants, foreign-decl
  visibility semantics beyond `#[foreign]` attribute carriage.

## Non-Goals

- Do not change the pretty-printer rendering shapes. All four bucket
  outputs reuse existing shapes per `pretty-printer.md` Specimen 120
  (foreign static) and the established Function / Struct / Enum /
  Union / Static / Const renderers.
- Do not introduce new `ItemKind` or `InstanceKind` variants. The
  six-variant closed alphabet at `spec.md:2205-2230` stays intact;
  bucket 4 surfaces as an Item-without-Instance (zero Instances per
  Item is already a permitted shape).
- Do not change foreign-decl semantics beyond surfacing them. The
  existing `Linkage::Foreign` flag continues to drive `#[foreign]`
  attribute rendering and any backend special-casing.
- Do not change the Rule 3 emission rule (Instance-iteration only).
  Item-only entries stay invisible to renderers and backends. This
  is by design and preserves the regseq-driver-56 pretty-printer
  guarantee.
- Do not modify `tests/fixtures/playground/`. Playground is the
  user's experimentation space; manual verification (Quality Gate)
  recompiles + inspects without modification.
- Do not touch the regseq-driver-56 call-site canonicalization
  (`canonicalize_call_site_callee` at `mod.rs:1978-2019`) or the
  regseq-driver-57 diag_log recognizer cleanup. Both are unrelated
  surfaces.
- Do not retire `Linkage::Foreign`. The flag continues to be set by
  Change B for foreign Items and stays the canonical signal for
  bucket 2.
- Do not address layout-of-generic-ADT-with-zero-monos. Per Rule 2
  bucket 3, generic ADTs without concrete monomorphizations have
  zero Instances and are not laid out.

## Test Plan

All tests in `crates/regseq-driver/tests/` (integration tests bridging
real source via rustc-driver) or `crates/regseq-model/tests/` (e2e
golden snapshots) unless noted otherwise.

Before-fix expectation: B1, B2, B3, B6, B7, B8, B9 fail (asymmetric
enumeration). After-fix: all of B1-B10 pass.

### B1: HIR enumeration completeness across all 6 ItemKinds

**New file:** `crates/regseq-driver/tests/bridge_eager_hir_enumeration.rs`.

Bridge a fixture exercising every ItemKind:

```rust
pub struct S { x: i32 }
pub enum E { A, B }
pub union U { f: u32, g: f32 }
pub static FOO: u32 = 1;
pub const BAR: u32 = 2;
pub fn baz() {}
```

Assert every source-derived top-level def has a corresponding Item in
`model.items` with the right kind:

- `S` -> `ItemKind::Struct`, `linkage == Default`, `is_generic == false`.
- `E` -> `ItemKind::Enum`, `linkage == Default`, `is_generic == false`.
- `U` -> `ItemKind::Union`, `linkage == Default`, `is_generic == false`.
- `FOO` -> `ItemKind::Static { Immutable }`, `linkage == Default`.
- `BAR` -> `ItemKind::Const`.
- `baz` -> `ItemKind::Function`, `is_generic == false`,
  `signature_non_rigid == false`.

### B2: Bucket 1 -- non-generic + sig-rigid + non-foreign -> exactly 1 Instance

**Same file as B1.**

Per Item from B1, assert exactly 1 Instance exists. For ADTs, the
Instance's `layout` is `Some(_)` with concrete size + align. For
fns, the Instance's `fn_sig` resolves to a concrete `FnSig` row;
`body == Some(_)` for source-defined fns.

### B3: Bucket 2 -- foreign -> exactly 1 Instance with body/init None

**New file:** `crates/regseq-driver/tests/bridge_foreign_enumeration.rs`.

Bridge a fixture with `extern "C" { fn extern_helper(x: u32) -> u32;
static EXTERN_VALUE: u32; }`. Assert:

- `extern_helper` Item: `ItemKind::Function`, `linkage == Foreign`.
- `EXTERN_VALUE` Item: `ItemKind::Static { Immutable }`,
  `linkage == Foreign`.
- Each has exactly 1 Instance.
- `extern_helper` Instance: `body == None`,
  `flavor == FunctionInstanceFlavor::Item`.
- `EXTERN_VALUE` Instance: `init == None`.

### B4: Bucket 3 -- generic ADT/fn with zero monomorphizations -> 0 Instances + Item present

**New file:** `crates/regseq-driver/tests/bridge_generic_bucket.rs`.

Bridge a fixture with one generic example per ItemKind (Struct,
Enum, Union, Function):

```rust
pub struct Slot<T>(pub T);
pub enum Either<L, R> { Left(L), Right(R) }
pub union Bag<T: Copy> { a: T, b: u32 }
pub fn id<T>(x: T) -> T { x }
```

Assert each Item is present with `is_generic == true` and ZERO
Instances. Then add `pub fn use_slot() -> Slot<u32> { Slot(7) }` to
force one monomorphization. Re-bridge and assert:

- All four generic Items still present.
- `Slot` now has exactly 1 Instance keyed on
  `Ty::Struct { item: Slot, args: [u32], layout }`.
- `Either`, `Bag`, `id` still have zero Instances (no
  monomorphizations forced).
- `use_slot` Item + Instance present (bucket 1).

Note: generic associated `const`s are out of scope -- Rust forbids
them at HIR level. Generic `static`s are also forbidden per spec
§9.4 statics-are-not-generic rule at `spec.md:2671-2673`. So bucket
3 applies only to Function/Struct/Enum/Union; B4 covers all four.

### B5: Bucket 4 -- non-generic + signature-non-rigid (RPIT) -> 0 Instances + Item present

**Anchor**: existing fixture at
`crates/regseq-driver/tests/bridge_synth.rs:399`
(`pub fn closure_fixture() -> impl Fn(u32) -> u32`).

Add an explicit test (either in `bridge_synth.rs` or in a new file)
asserting that AFTER the task lands:

- `closure_fixture` Item present (`ItemKind::Function`,
  `is_generic == false`, `signature_non_rigid == true`).
- No Function Instance for `closure_fixture`.
- The validator (`validate_bridge_completeness`) accepts the model
  (bucket 4 carve-out).

### B6: validator rejects bucket-1 partial coverage

**New file:** `crates/regseq-model/tests/validate_bridge_completeness_buckets.rs`.

Hand-construct a model with a bucket-1 Item (non-generic + sig-rigid +
non-foreign Struct) but ZERO Instances. Assert
`validate_bridge_completeness()` returns
`Err(InvariantViolation::ItemMissingInstance { .. })`.

Repeat for bucket-2 (foreign Function or Static without Instance) ->
error.

Hand-construct a bucket-1 model with TWO Instances for the same
ItemId -> error (the "exactly 1" half of the bucket-1 contract).

Hand-construct a bucket-2 model where the foreign Function Instance
has `body: Some(_)` (or foreign Static Instance has `init: Some(_)`)
-> error (the bucket-2 `body: None` / `init: None` invariant from
spec §9.4 at `spec.md:2642-2648` and `:2668-2677` must be enforced).

Hand-construct a bucket-3 model (generic Item with 0 Instances) ->
`Ok(())`.

Hand-construct a bucket-3 model (generic Item with 2 Instances via
distinct `GenericArgs`) -> `Ok(())` (generic Items may have 0+
Instances).

Hand-construct a bucket-4 model (Function Item with
`signature_non_rigid == true`, 0 Instances) -> `Ok(())`.

Hand-construct a bucket-4 model (Function Item with
`signature_non_rigid == true`, 1 Instance) -> error (bucket 4
requires zero Instances; a non-rigid signature cannot produce one).

### B7: e2e -- playground-shape fixture produces all expected outputs

**Anchor**: Change I e2e fixture + snapshot.

Cited above. Snapshot pinned.

### B8: e2e -- foreign decls render per Specimen 120

**Anchor**: extends B7's fixture.

Snapshot asserts:

- `#[foreign]\nfn ...::extern_helper(_1: u32) -> u32;` (no body).
- `#[foreign]\nstatic ...::EXTERN_VALUE: u32;` (no init).

### B9: e2e -- default trait method bodies surface as Function Items

**New file:** `crates/test-ir-fixtures/fixtures/trait_default_method.rs`
+ `crates/regseq-model/tests/e2e_trait_default_method.rs`.

Fixture:

```rust
pub trait T {
    fn m(&self) -> u32 { 42 }
}
pub struct S;
impl T for S {}
pub fn entry(s: &S) -> u32 { s.m() }
```

Assert:

- Trait default body `T::m` surfaces as `ItemKind::Function` Item at
  the trait's DefPath (Change H).
- The default body Function Instance has `body == Some(_)` and renders
  with the literal `42_u32`.
- `entry`'s call site resolves to either the trait-default Function
  Instance OR an impl-method Function Instance (whichever rustc
  produces via `Instance::resolve`). Document the observed shape in
  the snapshot.

### B10: workspace test suite stays green

`cargo test --workspace` green. Specifically watch:

- All regseq-driver-55 / -56 / -57 coverage tests stay green.
- `validate_item_instance_coverage.rs` tests pass with the new
  four-bucket validator.
- `bridge_pipeline.rs` Function-Instance counts may shift (more
  Items + more bucket-1 Instances); update assertions to match
  the new contract.
- `bridge_synth.rs::closure_fixture` (bucket 4) stays accepted
  by the validator.

## Review

### Files to send to Codex (`scripts/python/codex_runner.py`)

- This file (`.harness/tasks/regseq-driver/58-eager-hir-enumeration-of-source-top-level-items.md`).
- The Change H spec amendment text (inlined in the review prompt).

### Reviewer focus

- **Contract integrity.** Do Changes A-H together realize the
  locked Q1-Q5 contract? Cite the rule (1-4) each change anchors.
- **Bucket-coverage completeness.** Does B1-B6 collectively assert
  every bucket for every ItemKind? Flag any missing bucket-kind
  intersection.
- **Foreign-decl resolver.** Does Change B correctly avoid
  `source_item_def_id` and use a HIR-driven resolver that covers
  foreign items?
- **Generic Item handling.** Does Change A/C correctly intern
  generic Items WITHOUT triggering layout/sig translation? Confirm
  bucket-3 Items have no Instance.
- **RPIT carve-out detection.** Does Change E correctly detect the
  signature-non-rigid case via `translate_fn_sig` rejection? Cite
  the relevant `BridgeError` variant and pattern-match site.
- **Validator rewrite.** Does Change G implement the four-bucket
  classification without losing the regseq-driver-55-v3 RPIT
  carve-out semantics? Cite `model.rs` line ranges.
- **Schema impact.** Change G introduces `is_generic: bool` and
  `signature_non_rigid: bool` flags on Item (or a side-table). Is
  this schema touch in scope? Per Non-Goals "no new ItemKind /
  InstanceKind variants" -- flags on existing Item are NOT new
  variants. Flag if this interpretation differs from the implementer's.
- **Citation drift.** Every file:line in this spec must anchor its
  claim against HEAD.
- **Scope creep.** Per CLAUDE.md "No features beyond what was
  asked" + "Touch only what you must". Does Change I e2e fixture
  scope match what B1-B10 actually exercise? Flag any unrequested
  scaffolding.
- **No defensive fallbacks.** Per `feedback_no_defensive_fallbacks`.
  Any branch that "shouldn't fire" must produce a `BridgeError`,
  not a silent pass-through. Audit Changes D / E / F for this.

### Adversarial prompt template

```
<task>
Review .harness/tasks/regseq-driver/58-eager-hir-enumeration-of-source-top-level-items.md
plus the inlined Change H spec amendment. Identify:
- Citation drift (any file:line that does not anchor the claim).
- Bucket-coverage gaps in the Test Plan.
- Implementation feasibility for each Change (A-H).
- Schema-impact scope per Non-Goals.
- Scope creep beyond the locked Q1-Q5 contract.
</task>

<grounding_rules>
- Cite file:line for every claim.
- Mark hypotheses as hypotheses.
- Don't speculate about code or spec sections you didn't read.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED WITH MINOR FIXES | NOT READY
PER-CHANGE VERIFICATION: A, B, C, D, E, F, G, H, I, J bullets with file:line
RULE-COVERAGE MATRIX: Rule 1 / 2 / 3 / 4 -- which Changes anchor each
BUCKET-COVERAGE MATRIX: bucket 1 / 2 / 3 / 4 across ItemKind variants -- which tests
SCHEMA IMPACT: in-scope per Non-Goals? PASS / FAIL
CITATION DRIFT: list or "none"
SCOPE-CREEP CHECK: pass/fail with citation
NO-DEFENSIVE-FALLBACK AUDIT: pass/fail per Change
TEST PLAN VERIFICATION: B1, B2, B3, B4, B5, B6, B7, B8, B9, B10 bullets
NEW ISSUES: numbered list or "none"
REQUIRED FIXES: numbered list (only if VERDICT != APPROVED)
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## Quality Gate

- `cargo fmt --all` clean.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo test --workspace` green (run with `--test-threads=1` if the
  rustc-driver-test stderr-interleaving quirk surfaces).
- Manual verification on `tests/fixtures/playground/` (with the
  user's current `Reg(u32) + impl + entry()` fixture): rebuild
  `cargo build -p regseq-driver -p cargo-regseq`, recompile via
  `cargo regseq --target-os linux --platform-file platform.toml
  --emit model`, inspect `regseq-out/model.txt`:
  - `struct playground::Reg(u32);` (or `struct playground::Reg { 0:
    u32 }` -- whichever the pretty-printer produces for tuple structs)
    is present.
  - `fn playground::{impl#0}::new(_1: u32) -> playground::Reg` is
    present with full body.
  - `const playground::{impl#0}::REG_0`, `::REG_1` continue to render
    (already worked pre-fix).
  - `fn playground::entry() -> ()` renders (already worked pre-fix).
  - Total Item count = 5; total Instance count = 5.
- Codex Task Review verdict: APPROVED or APPROVED WITH MINOR FIXES.
  Zero unaddressed BLOCKERs across review passes.

## Acceptance Criteria

1. **HIR walk completeness (Rule 1)**: the bridge enumerates Items
   eagerly via a single HIR-driven pass covering `DefKind::Const |
   AssocConst | Static | Fn | AssocFn (with body) | Struct | Enum |
   Union | ForeignFn | ForeignStatic`. Closures and trait
   declarations are not enumerated by the eager pass.

2. **Bucket 1 Instance coverage (Rule 2)**: every non-generic +
   signature-rigid + non-foreign source-derived Item with
   `kind in {Function, Static, Const, Struct, Enum, Union}` has
   exactly 1 Instance after `run_bridge` completes.

3. **Bucket 2 Instance coverage (Rule 2)**: every foreign Item has
   exactly 1 Instance with `body: None` (Function) or `init: None`
   (Static) per spec §9.4.

4. **Bucket 3 zero-mono behavior (Rule 2)**: generic Items
   (`is_generic == true`) with zero monomorphizations have 0
   Instances. The validator accepts this.

5. **Bucket 4 signature-non-rigid carve-out (Rule 2)**:
   `closure_fixture`-shape fns (non-generic + signature-non-rigid)
   have 0 Instances and `signature_non_rigid == true` on the Item.
   The validator accepts this. The regseq-driver-55-v3 blanket
   Function carve-out is removed.

6. **Rule 3 emission unchanged**: pretty-printer iterates Instances
   only; Item-only entries (buckets 3, 4) are invisible in rendered
   output.

7. **Rule 4 validator rewrite**: `validate_bridge_completeness`
   classifies each non-synthetic Item via `classify_bucket()` and
   enforces the per-bucket Instance-count rule. The
   `is_foreign`-skip and blanket Function-skip at
   `crates/regseq-model/src/model.rs:3475-3486` are gone.

8. **Schema flags on Item**: `is_generic: bool` and
   `signature_non_rigid: bool` flags are populated by the eager HIR
   walk + Phase-5 Instance attempt. Implementer may use a side-table
   instead; the validator's classifier signature is identical either
   way.

9. **Change H spec amendment lands**: `docs/discussions/regseq-model/spec.md`
   §8.1 records the default-trait-method-body Item-shape clarification.

10. **Tests B1-B10 pass**: per the Test Plan; full workspace suite
    green.

11. **Manual playground verification**: the user's stress fixture
    (`tests/fixtures/playground/src/lib.rs` with `Reg(u32) + impl
    Reg { const REG_0; const REG_1; fn new(off) -> Reg } + pub fn
    entry()`) renders all 5 items in `regseq-out/model.txt`.

12. **Harness expectations file** (`.harness/expectations/regseq-driver-58.md`)
    records before/after evidence with file:line citations.
