# regseq-driver-55: Phase-5 Instance-construction completeness for source and vtable-type Items

## Description

Close the Phase-5 Instance-construction completeness gap surfaced by
manual inspection of pretty-printed RegseqModel output against a
fixture defining top-level `struct` / `enum` / `union` / `static` /
`const` items, and independently confirmed by an adversarial Codex
investigation: the bridge's Phase-5 orchestrator
(`crates/regseq-driver/src/bridge/mod.rs::construct_instances_and_resolve_cross_references`)
constructs Instances for *some* `ItemKind` variants
(`construct_function_instances` for source `fn`,
`construct_closure_call_instances` for the closure family,
`construct_const_memory_static_instances` for reference-typed const
memory, plus `synth::rekey_synthesis_in_phase5` for vtable instances)
but omits four `ItemKind` -> `InstanceKind` paths the spec requires.

Per `spec.md` §3.4 Phase 5 (`spec.md:1226-1229`):
"Bridge populates `instances` per §9.2 (one Instance per (Item,
GenericArgs, rustc shim flavor) tuple)." Per §9.4 (`spec.md:2576-2660`)
`InstanceKind` is a closed 6-variant set; per §8.1
(`spec.md:2351-2356`) the `ItemKind` alphabet mirrors `InstanceKind`.
The four missing paths are:

1. **Source `struct Foo` -> `InstanceKind::Struct { layout }`.** Today
   `bridge/ty.rs::translate_adt` (`ty.rs:286-324`) interns the
   `ItemKind::Struct` Item and the `Ty::Struct { item, args, layout }`
   view, but no Phase-5 pass pushes the symmetric Instance. A
   minimal reproduction has `pub struct Point { x: i32, y: i32 }`
   plus `pub fn add(p: &Point, q: &Point) -> ...` -- rendered output
   contains `&...::Point` inside `add`'s signature (so the Item + Ty
   exist) but no `struct ...::Point` definition appears, because
   `PrettyPrinter::render` (`pretty-printer.md` §10.3) iterates
   `model.instances`, not `model.items`.

2. **Source `enum Foo` -> `InstanceKind::Enum { layout }`.** Symmetric
   gap. `regseq-driver-54` recently shipped enum *layout* construction
   in `bridge/ty.rs`, but the Phase-5 Instance side was not authored.

3. **Source `union Foo` -> `InstanceKind::Union { layout }`.**
   Symmetric gap.

4. **Synth `VTableType` Items (`ItemKind::Struct`) ->
   `InstanceKind::Struct { layout }`.** `bridge/synth/vtable.rs`
   produces the `VTableType` Item and its parallel `Ty::Struct` view
   (per §13.2 + §13.4); `synth/vtable.rs::push_vtable_instance_statics`
   pushes one `InstanceKind::Static` per `VTableInstance` Item, but no
   pass pushes the `InstanceKind::Struct` definition Instance for the
   `VTableType` Item itself. Same pretty-printer consequence: vtable
   type definitions never render.

5. **Source `static FOO: T = ...;` -> `InstanceKind::Static { ty,
   init }`.** Phase-2 walker
   (`bridge/mod.rs::translate_with_interleaved_synthesis`,
   `mod.rs:669-680`) filters to `MonoItem::Fn(_)` and skips every
   other rustc `MonoItem` variant, so source statics are not even
   interned as Items. Per `spec.md` §8.1 + §9.4 they must surface as
   `ItemKind::Static { mutability }` -> `InstanceKind::Static { ty,
   init: Some(_) }` (or `init: None` for foreign decls, but foreign
   handling is out of scope per Non-Goals).

6. **Source `const FOO: T = ...;` -> `InstanceKind::Const { ty,
   value }`.** Rustc does not surface `const` items via mono-item
   discovery (they are erased at codegen-time). The bridge must
   enumerate top-level `const` items separately via
   `tcx.hir_crate_items(()).definitions()` filtered to
   `DefKind::Const`, intern the `ItemKind::Const` Item, and push the
   `InstanceKind::Const { ty, value }` Instance.

After this task lands:

- Every source-defined `struct` / `enum` / `union` / `static` /
  `const` at the source crate's top-level surfaces as a corresponding
  `Instance` and renders via `pretty-printer.md` §10.2.2-§10.2.6.
- Every synthesised `VTableType` `Item` carries a parallel
  `InstanceKind::Struct { layout }` `Instance` so vtable type
  definitions render at use-sites.
- A new bridge-completeness entry point on `RegseqModel`
  (`validate_bridge_completeness`) is invoked from `run_bridge`
  and catches any future regression where a source-derived
  non-foreign `Item` exists without its required Instance(s).
  This is a separate entry point from `RegseqModel::validate`
  so the existing partial-model validator test corpus stays
  intact.
- The new `test-ir-fixtures` fixture + integration test assert
  the Instance shapes and rendered form for each source
  kind end-to-end.

Foreign declarations (`extern "C" { fn foo(); }`,
`extern "C" { static FOO: u32; }`) are explicitly out of scope: Codex's
investigation flagged the surface as SPEC-AMBIGUOUS, and resolving the
ambiguity (eager vs lazy discovery; `init: None` rendering shape)
requires a spec clarification round before implementation.

## Input Context

- `docs/discussions/regseq-model/spec.md` §3.4 Phase 5
  (`spec.md:1226-1280`) -- the (Item, GenericArgs, rustc shim flavor)
  cardinality rule for `model.instances`.
- `docs/discussions/regseq-model/spec.md` §8.0 (`spec.md:2125-2230`)
  -- the closed 6-variant `ItemKind` alphabet.
- `docs/discussions/regseq-model/spec.md` §8.1
  (`spec.md:2332-2362`) -- impl-lifting rules + the explicit
  6-variant mirror with `InstanceKind`.
- `docs/discussions/regseq-model/spec.md` §9.4
  (`spec.md:2576-2660`) -- the `InstanceKind` closed variant set,
  including `Struct { layout }` / `Enum { layout }` /
  `Union { layout }` / `Static { ty, init }` / `Const { ty, value }`.
- `docs/discussions/regseq-model/spec.md` §3.1 invariant 10
  (`spec.md:772-781`) -- `Function.body.is_none() iff linkage ==
  Linkage::Foreign` biconditional. The companion
  `Static.init.is_none() iff foreign` rule is grounded
  separately at `spec.md:2626-2628` (§9.4's per-variant
  `InstanceKind::Static` documentation), not in invariant 10
  itself.
- `docs/discussions/regseq-model/spec.md` §3.1 invariant 14
  (`spec.md:760-770`) -- `InstanceKind` layout consistency (the
  family this task's new validator check extends).
- `docs/discussions/regseq-model/spec.md` §13.2
  (`spec.md:4589-4814`) -- vtable family including the synthetic
  layout algorithm; the `VTableType` Item is the load-bearing source
  of the new `InstanceKind::Struct { layout }` row.
- `docs/discussions/regseq-model/pretty-printer.md` §10.2.4-§10.2.6
  (`pretty-printer.md:3813-4039`) -- ADT-definition rendering rules
  that the new Instances will dispatch through.
- `docs/discussions/regseq-model/pretty-printer.md` §10.3
  (`pretty-printer.md:4201-4255`) -- top-level `render` iterates
  `model.instances` only; this is why missing Instances render as
  nothing.
- `crates/regseq-driver/src/bridge/mod.rs:1026-1095` -- the Phase-5
  orchestrator
  (`construct_instances_and_resolve_cross_references`) where the
  new construction passes slot in.
- `crates/regseq-driver/src/bridge/mod.rs:1097-1196`
  (`construct_function_instances`) -- the symmetric source-Function
  precedent.
- `crates/regseq-driver/src/bridge/mod.rs:1217-1344`
  (`construct_closure_call_instances`) -- the `InstanceKind::Struct
  { layout }` push pattern at lines 1336-1344 is the precedent to
  mirror.
- `crates/regseq-driver/src/bridge/mod.rs:640-703`
  (`translate_with_interleaved_synthesis`) -- the Phase-2 walker
  that filters to `MonoItem::Fn`; this filter is extended to also
  handle `MonoItem::Static`.
- `crates/regseq-driver/src/bridge/ty.rs:286-324` -- `translate_adt`
  produces the `ItemKind::Struct/Enum/Union` Items the new pass
  walks.
- `crates/regseq-driver/src/bridge/ty.rs:454-463` -- linkage default
  (`Linkage::Default`); non-Foreign Items must produce `init: Some`
  Instances.
- `crates/regseq-driver/src/bridge/synth/vtable.rs:32` -- VTableType
  Item creation (`ItemKind::Struct`).
- `crates/regseq-driver/src/bridge/synth/vtable.rs:294-330`
  (`push_vtable_instance_statics`) -- the existing Static-side push;
  the new Struct-side push slots alongside.
- `crates/regseq-driver/src/bridge/const_.rs:202-233`
  (`construct_const_memory_static_instances`) -- the existing
  source-Static-adjacent Instance constructor; structurally
  analogous to the new source-static pass.
- `crates/regseq-model/src/model.rs:1594-1619`
  (`RegseqModel::validate`) -- the model-correctness entry
  point this task DELIBERATELY does NOT fold the new
  Item-vs-Instance coverage check into; the new
  `validate_bridge_completeness` method is a separate sibling
  entry point invoked from `run_bridge`.
- `crates/regseq-model/src/model.rs:3332-3355` (`validate_instance`)
  -- the existing per-Instance walker (does not catch missing
  Instances).
- `crates/regseq-model/src/layout_validator.rs:722-750`
  (`validate_instance_layouts`) -- the existing invariant-14
  validator family the new check extends.
- `crates/test-ir-fixtures/fixtures/enum_round_trip.rs` -- the
  reference shape for an end-to-end fixture under the canonical
  cross-crate fixture home (per the phase-1a brief's Axis-5
  `test-ir-fixtures` scaffold). The new
  `instance_completeness.rs` fixture this task adds follows the
  same `#![no_std]` shape and per-arm comment convention.
- `crates/regseq-model/tests/e2e_enum_round_trip.rs` -- the
  matching integration-test shape; the new
  `e2e_instance_completeness.rs` test mirrors it.

## Implementation Notes

### Change A: Phase-2 walker extension for source `static` items

`crates/regseq-driver/src/bridge/mod.rs` around line 669, extend
the first pre-pass loop in `translate_with_interleaved_synthesis`
so a `MonoItem::Static(def_id)` interns an `ItemKind::Static`
Item alongside the existing `ItemKind::Function` interning:

```rust
for mono_item in &discovered_mono_items {
    match mono_item {
        MonoItem::Fn(instance) => {
            let rustc_middle::ty::InstanceKind::Item(def_id) = instance.def else {
                continue;
            };
            if !is_source_derived_function(ctxt.input.tcx, def_id) {
                continue;
            }
            let _ = ty::intern_item(ctxt, def_id, ItemKind::Function, ty::SpecialAdt::None);
        }
        MonoItem::Static(def_id) => {
            let mutability = source_static_mutability(ctxt.input.tcx, *def_id);
            let _ = ty::intern_item(
                ctxt,
                *def_id,
                ItemKind::Static { mutability },
                ty::SpecialAdt::None,
            );
        }
        MonoItem::GlobalAsm(_) => continue, // out of scope; §15.1 rejection at use sites
    }
}
```

`source_static_mutability` reads
`ctxt.input.tcx.static_mutability(def_id)` and maps it to the
RegseqModel `Mutability` enum. The existing `intern_item`
dedup-by-DefPathHash machinery applies unchanged.

### Change B: Phase-2 walker extension for source `const` items

`const` items are not surfaced by rustc's mono-item discovery (they
are erased at codegen time). Add a third pre-pass walking
`tcx.hir_crate_items(()).definitions()` filtered to both
`DefKind::Const` AND `DefKind::AssocConst`, interning each as
`ItemKind::Const`. Per `spec.md:2334-2346` (especially line 2345)
associated constants lift to top-level `Const` Items; both forms
share the same `ItemKind::Const` shape and must surface as Items.
Use `intern_item` for idempotent dedup-by-DefPathHash; both walks
into the same Item table are safe by construction.

### Change C: `construct_source_static_instances`

New helper in `crates/regseq-driver/src/bridge/mod.rs` slotted into
`construct_instances_and_resolve_cross_references` between
`construct_function_instances` and `construct_closure_call_instances`
(rationale: source statics must be constructed before any
synth/body pass that might reference them). Walks
`model.items.iter_enumerated()` filtered to
`ItemKind::Static { .. }` items whose `item_synthetic_kinds[id]
== SyntheticItemKind::None` (i.e. source-derived, not synth
VTableInstance or ConstMemoryStatic). For each:

- Resolve the rustc `DefId` via the existing
  `source_item_def_id(ctxt, item_id)` helper.
- Translate the static's type via `ty::translate_ty(ctxt,
  tcx.type_of(def_id).instantiate_identity())` to a `TyId`.
- Translate the initialiser via the existing const-translation
  path: `const_::translate_static_init(ctxt, def_id)` producing
  `Option<ConstId>` (`Some(_)` for source-defined statics with a
  `mir_for_ctfe` initialiser; the foreign-static `None` path is
  out of scope per Non-Goals).
- Push `Instance { item: item_id, generic_args: vec![],
  emission_name, span, kind: InstanceKind::Static { ty, init } }`.
  Statics cannot be generic per §8.0 (`spec.md:2249-2254`); the
  empty `generic_args` is correct.

### Change D: `construct_source_const_instances`

New helper slotted immediately after `construct_source_static_instances`.
Walks `model.items.iter_enumerated()` filtered to `ItemKind::Const`
with `SyntheticItemKind::None`. For each, resolve the rustc DefId,
translate `ty` and `value` (via `const_::translate_const(ctxt,
tcx.const_eval_poly(def_id))`), push `Instance { ..., kind:
InstanceKind::Const { ty, value } }`.

Cardinality rule: like statics, consts cannot be generic; one
Instance per Item.

### Change E: `construct_nominal_adt_instances`

New helper slotted immediately after `construct_source_const_instances`
and before `construct_closure_call_instances` (rationale: the
closure-captures path at `mod.rs:1336-1344` already creates
`InstanceKind::Struct { layout }` for the `ClosureCaptures` synth
Item via a per-closure cache; both paths share the same Instance
shape but key on disjoint Item sets, so order does not matter for
correctness -- we slot this before for symmetry with Function /
Static / Const ordering).

Algorithm:

1. Walk `model.items.iter_enumerated()`.
2. Filter to items whose `kind ∈ { ItemKind::Struct, ItemKind::Enum,
   ItemKind::Union }` AND whose
   `model.item_synthetic_kinds[item_id]` matches one of:
   - `SyntheticItemKind::None` (source-derived ADT).
   - `SyntheticItemKind::VTableType { .. }` (synth vtable type; the
     `Struct` Instance is produced here, not in `synth/vtable.rs`).

   Filter OUT `SyntheticItemKind::ClosureCaptures { .. }` -- those
   are owned by `construct_closure_call_instances` per the
   precedent at `mod.rs:1336-1344` and the per-closure cache there
   keys on closure DefPathHash. Producing them here would conflict
   with the existing dedup guard.
3. For each surviving Item: enumerate the monomorphisations by
   walking `model.types` for every `Ty::Struct/Enum/Union { item:
   item_id, args, layout }` entry. Phase-2's mono-item-driven
   type translation populates one Ty entry per reachable
   `(item, args)` pair, so this walk is the authoritative
   enumeration -- no rustc DefId resolution is required at this
   step. The cardinality rule (§3.4 Phase 5: "one Instance per
   (Item, GenericArgs, ...) tuple") produces one Instance per
   matching Ty entry. For VTableType ADTs the synth machinery
   has already populated the parallel `Ty::Struct { item:
   item_id, args, layout }` entry per §13.2 + §13.4, so the same
   walk covers both source and synth ADT origins.

   Per Ty entry:
   - Compute `generic_args` from the Ty's `args` field (the
     translated `GenericArgs` already in canonical model form).
   - Compute `emission_name` and `span` from
     `model.items[item_id]`.
   - Push `Instance { item: item_id, generic_args, emission_name,
     span, kind: match item.kind { Struct => Struct { layout: ty_id
     }, Enum => Enum { layout: ty_id }, Union => Union { layout:
     ty_id }, _ => unreachable!() } }`.

   Do NOT call `source_item_def_id(ctxt, item_id)`
   (`bridge/mod.rs:1529-1539`): that helper scans
   `tcx.hir_body_owners()`, which covers HIR body owners
   (functions, statics, consts -- which is why Changes C and D
   can use it) but does NOT include ADT definitions (`struct` /
   `enum` / `union` have no HIR body). For ADTs the `model.types`
   walk above is the authoritative enumeration and avoids the
   rustc DefId dependency entirely.
4. Idempotency: if an `Instance` with key `(item_id, args_key)`
   already exists in `model.instances` (e.g. a future fixture
   re-enters this helper), skip the push. Use the canonical
   `args_key_for_generic_args_with_pending` helper for keying.

### Change F: Slot ordering in `construct_instances_and_resolve_cross_references`

The final Phase-5 orchestrator ordering (after this task):

```rust
fn construct_instances_and_resolve_cross_references(ctxt) -> Result<...> {
    synth::rekey_synthesis_in_phase5(ctxt)?;
    construct_function_instances(ctxt)?;
    construct_source_static_instances(ctxt)?;          // NEW (Change C)
    construct_source_const_instances(ctxt)?;           // NEW (Change D)
    construct_closure_call_instances(ctxt)?;
    body::run_phase5_body_passes(ctxt)?;
    if !ctxt.pending_items.is_empty() {
        sort::f4_sort(&ctxt.model, &mut ctxt.pending_items)?;
        assign::assign_late_pending_items(ctxt);
    }
    patch_resolved_source_ty_pending(ctxt);
    let item_remap = ctxt.model.sort_items_by_f4();
    ctxt.handles.remap_item_ids(&item_remap);
    const_::construct_const_memory_static_instances(ctxt)?;
    diag_log::dead_item_elim::eliminate_diag_log_peel_residue(ctxt)?;
    construct_nominal_adt_instances(ctxt)?;            // NEW (Change E) -- FINAL construction step
    Ok(())
}
```

`construct_source_static_instances` (Change C) and
`construct_source_const_instances` (Change D) slot between
`construct_function_instances` and `construct_closure_call_instances`
(the symmetric source-Function precedent).

`construct_nominal_adt_instances` (Change E) MUST run as the
**FINAL construction step**, after both
`patch_resolved_source_ty_pending` + `sort_items_by_f4`/remap and
`eliminate_diag_log_peel_residue`. Two independent constraints
force this slot:

(i) **Placeholder ItemId barrier.** Source `struct` / `enum` /
`union` Items are NOT rustc `MonoItem`s and are first interned
during Phase-5 fn-sig / body translation -- AFTER Phase 4. Their
`Ty::Struct/Enum/Union { item, args, layout }` entries therefore
carry the high-end placeholder `ItemId` (per
`placeholder_item_id_for_handle`) until
`patch_resolved_source_ty_pending` resolves them. Reading the
`Ty`'s `item` field before that resolution yields an unpatched
placeholder; pushing `Instance.item = <placeholder>` would panic
the closing `sort_items_by_f4` remap
(`remap[item.index()]` indexes past `model.items.len()`).

(ii) **DiagLogPeel residue barrier.** Running before
`eliminate_diag_log_peel_residue` would push an
`InstanceKind::Enum` for the DiagLogPeel residue `LogArg` enum,
pinning it in the model against `spec.md` §26.6's "the `LogArg`
type does NOT appear in RegseqModel" contract
(`I-DiagLog-CallArity` carrier-survival rule).

The final `ctxt.model.sort_instances_by_f4_2()` at the end of
`run_bridge` (`crates/regseq-driver/src/bridge/mod.rs:635`)
restores the F4-2 canonical ordering across the expanded
Instance set; no per-pass sort is needed.

### Change G: Validator extension -- Item-vs-Instance coverage (separate post-bridge entry point)

Add a new public method on `RegseqModel`:

```rust
pub fn validate_bridge_completeness(&self) -> Result<(), InvariantViolation>
```

The method enforces: for every `Item` at index `i` whose `kind` is
one of `{ Function, Static { .. }, Const, Struct, Enum, Union }`
AND whose `item_synthetic_kinds[i] == SyntheticItemKind::None`
AND whose `linkage != Linkage::Foreign`, there exists at least
one `Instance` in `model.instances` with `item == i`.

The new method is invoked from `bridge::run_bridge` AFTER the
final `model.sort_instances_by_f4_2()` call at
`crates/regseq-driver/src/bridge/mod.rs:635` (the existing
post-bridge invariant ratchet). It is also invoked explicitly
from Test Plan B1-B3 and from
`crates/regseq-model/tests/e2e_instance_completeness.rs` (in
addition to the existing `model.validate(&bridge_input)` call).

**The new method is DELIBERATELY NOT folded into
`RegseqModel::validate(&mut self, &BridgeInput)`.** Rationale:
`validate()` is the model-correctness entry point for arbitrary
models (hand-constructed partial test fixtures, deserialised
models, mid-construction snapshots); the partial-model
validator test corpus at
`crates/regseq-model/tests/{id_referential_integrity_validator,
layout_validator}.rs` legitimately constructs partial models
with no Instances to negative-test other invariants (notably
`F4OutOfOrderInstance` at
`id_referential_integrity_validator.rs:446`,
`InstanceLayoutMismatch` at `layout_validator.rs:227`, and
`InstanceLayoutItemArgsMismatch` at `layout_validator.rs:254`),
and positive layout tests like `layout_pass_struct` at
`layout_validator.rs:813-824` that build a one-Struct Item
zero-Instance model and rely on `validate()` returning `Ok(())`.
Forcing those models to carry Instances either masks the
intentional negative tests or requires a per-test corpus
migration the task spec did not scope. The new method is the
BRIDGE-COMPLETENESS entry point, distinct from model-correctness.
Both surfaces are load-bearing and must not be conflated.

Spec rationale: §3.4 Phase 5 + §3.1 invariant 14 (`spec.md:760-770`)
+ §8.1 (`ItemKind` mirrors `InstanceKind`). This is the missing
bridge-completeness invariant Codex's investigation flagged.

Synthetic-Item exemption (the
`item_synthetic_kinds[i] == None` clause): the DiagLogPeel
residue DCE (regseq-driver-48/49; Non-Goals 5 + 6)
legitimately strips Instances of synth Items, and the post-elim
model retains synth Items with no Instance. Synthetic-Item
Instance lifecycle is owned by the synth passes + residue DCE;
the new check exempts them.

Foreign-source-Item exemption (the `linkage != Foreign` clause):
foreign declarations have no body / init by definition per §3.1
invariant 10 (`spec.md:772-781`, Function body biconditional)
and §9.4 (`spec.md:2626-2628`, Static init biconditional);
whether they require an Instance row at all is SPEC-AMBIGUOUS
per Non-Goals 1-2.

### Conventions

- All new helpers in `bridge/mod.rs` follow the existing
  Phase-5-pass convention: take `&mut BridgeCtxt<'_>`, return
  `Result<(), BridgeError>`, no external state.
- New `BridgeError` variants only if a translation path can fail
  for reasons not already covered by existing variants. Reuse
  `BridgeError::TyTranslation`, `BridgeError::ConstTranslation`,
  etc., where applicable.
- No defensive fallbacks per CLAUDE.md / README.md §2 "No
  defensive fallbacks": the toolchain is pinned; if a path's
  precondition cannot hold per the spec, panic with a clear
  message rather than returning a fallback shape.
- All new doc comments cite `spec.md` by section number, not by
  line number (line numbers drift; section numbers do not), per
  the README §2 "Code does not reference discussion-folder paths"
  rule's allowance for stable architectural-decision tags. Where
  a citation is load-bearing, prefer the `D-{stage}-{decision}`
  form or the `Q1`-`Q7` form.

## Non-Goals

This task explicitly does NOT cover:

1. **Foreign `extern "C" { fn foo(); }` Function Items.** Codex
   flagged this as SPEC-AMBIGUOUS: eager-vs-lazy discovery
   strategy + the `body: None` rendering shape both need spec
   clarification. Filed as a forward expectation on the
   foreign-decl follow-up task.

2. **Foreign `extern "C" { static FOO: u32; }` Static Items.**
   Same SPEC-AMBIGUOUS framing. Filed as a forward expectation.

3. **Spec edits.** If a load-bearing ambiguity surfaces during
   implementation, file an inbound expectation on the relevant
   stage (per README §2 "Forward expectations") rather than
   editing spec.md / pretty-printer.md.

4. **Phase-1b backend codegen.** No C lowering of the new Instance
   rows. The pretty-printer side of rendering is what locks
   correctness here.

5. **Validator extension beyond Item-vs-Instance coverage.** The
   §3.1 invariant 10 biconditional under-enforcement Codex flagged
   (`model_tests.rs:279-307` accepting `body: None` without
   `Linkage::Foreign`) is a separate validator-side concern that
   should be addressed in its own `regseq-model-NN` follow-up
   task.

6. **Closure-family or ConstMemoryStatic Instance reorganisation.**
   The existing passes
   (`construct_closure_call_instances`,
   `construct_const_memory_static_instances`) stay unchanged;
   they own their respective slot in the Phase-5 orchestrator.

7. **Pretty-printer / IR-shape spec changes.** The pretty-printer
   already specifies definition-site rendering for `Struct` /
   `Enum` / `Union` / `Static` / `Const` Instances per
   `pretty-printer.md` §10.2.2-§10.2.6; this task changes only the
   bridge to populate the Instances those renderers dispatch on.

## Test Plan

### A. Unit tests in `crates/regseq-driver/tests/`

New file `bridge_phase5_instance_completeness.rs` with one
`#[test]` function per axis:

A1. **Source struct -> Struct Instance.** Construct a minimal
    `BridgeInput` for a fixture containing `pub struct Point { x:
    i32, y: i32 }` and `pub fn touch(p: &Point) -> i32 { p.x }`
    (the touch function forces Phase-2 Ty discovery of `Ty::Struct
    { item: Point, args: [], layout }`). Assert
    `model.instances` contains an entry with `item == <Point's
    ItemId>` and `kind == InstanceKind::Struct { layout: <Point's
    Ty::Struct TyId> }`.

A2. **Source enum -> Enum Instance.** Same shape with
    `pub enum State { Idle, Active(u32), Error }` and a
    `pub fn classify(s: &State) -> u32` discriminator.

A3. **Source union -> Union Instance.** Same shape with
    `pub union Pair { x: u32, y: i32 }` plus a touch function.

A4. **Generic struct produces one Instance per monomorphisation.**
    `pub struct Wrap<T> { v: T }` plus `pub fn use_u32(w: &Wrap<u32>)
    -> u32 { w.v }` and `pub fn use_u64(w: &Wrap<u64>) -> u64 { w.v }`.
    Assert two `InstanceKind::Struct { layout }` rows whose `layout`
    TyIds resolve to distinct `Ty::Struct { args }` entries.

A5. **Source static -> Static Instance.** Fixture
    `pub static COUNTER: u32 = 0;`. Assert
    `InstanceKind::Static { ty, init: Some(_) }` row whose `init`
    resolves to a `Const::Int(0)` or equivalent.

A6. **Source `static mut` -> Static Instance with `Mutability::Mut`.**
    Fixture `pub static mut FLAG: u32 = 0;`. Assert the parent
    `ItemKind::Static { mutability: Mutability::Mut }`.

A7. **Source const -> Const Instance.** Fixture
    `pub const LIMIT: u32 = 64;`. Assert
    `InstanceKind::Const { ty, value }` row.

A8. **Synth VTableType -> Struct Instance.** Fixture
    `pub trait Reader { fn read(&self) -> u32; } pub struct
    Concrete; impl Reader for Concrete { fn read(&self) -> u32 { 0
    } } pub fn dispatch(r: &dyn Reader) -> u32 { r.read() }`.
    Assert that for the synth `VTableType` Item, an
    `InstanceKind::Struct { layout }` row exists in
    `model.instances`.

A9. **Idempotency.** Re-invoke `construct_nominal_adt_instances`
    against an already-populated model; assert no duplicate
    Instance rows. (Implementation: extract the loop body into a
    helper, call twice from a test harness.)

### B. Bridge-completeness coverage unit test in `crates/regseq-model/tests/`

New file `validate_item_instance_coverage.rs`. Tests invoke the
new `validate_bridge_completeness` entry point added by Change G,
NOT the `validate(&BridgeInput)` global entry point. The
partial-model corpus at
`crates/regseq-model/tests/{id_referential_integrity_validator,
layout_validator}.rs` is intentionally left untouched and
continues to call `validate(&BridgeInput::matching(model))` per
the existing API at `crates/regseq-model/src/model.rs:1532`.

B1. **Negative regression.** Construct a `RegseqModel` by hand
    with one `ItemKind::Struct` Item, `SyntheticItemKind::None`,
    `linkage: Linkage::Default`, and ZERO matching Instance.
    Assert `model.validate_bridge_completeness()` returns an
    error variant naming the missing coverage. Verify in the
    same test that `model.validate(&BridgeInput::matching(&model))`
    returns `Ok(())` for the SAME model -- this is the
    cross-check that the two entry points are deliberately
    distinct (the model-correctness path is permissive about
    Item-vs-Instance coverage; the bridge-completeness path is
    strict).

B2. **Foreign carve-out (positive).** Same as B1 but with
    `linkage: Linkage::Foreign`. Assert
    `model.validate_bridge_completeness()` returns `Ok(())` --
    foreign Items legitimately may have no Instance per the
    foreign-decl carve-out at Non-Goals 1-2 in this task
    (foreign `fn` and foreign `static` deferred as
    SPEC-AMBIGUOUS).

B3. **Source-derived positive path.** Construct a `RegseqModel`
    by hand with one `ItemKind::Struct` Item,
    `SyntheticItemKind::None`, `linkage: Linkage::Default`, AND
    one matching `InstanceKind::Struct { layout }` Instance
    whose `item` field references that Item. Assert
    `model.validate_bridge_completeness()` returns `Ok(())`.
    Without this case, B1 alone could pass under a check that
    rejects every non-foreign Item regardless of Instance
    presence.

### C. End-to-end via `test-ir-fixtures`

C1. Add a new fixture
    `crates/test-ir-fixtures/fixtures/instance_completeness.rs`
    following the `#![no_std]` shape used by
    `crates/test-ir-fixtures/fixtures/enum_round_trip.rs`. The
    fixture exercises every in-scope source kind exactly once
    plus the synth-VTableType path:

    ```rust
    #![no_std]

    pub static COUNTER: u32 = 0;
    pub static mut FLAG: u32 = 0;
    pub const LIMIT: u32 = 64;

    pub struct Point { x: i32, y: i32 }
    pub enum State { Idle, Active(u32), Error }
    pub union Pair { pub x: u32, pub y: i32 }
    pub struct Wrap<T> { v: T }

    pub fn touch_point(p: &Point) -> i32 { p.x }
    pub fn touch_pair(p: &Pair) -> u32 {
        // Safe field-access on a `union` shape forces Phase-2
        // discovery of `Ty::Union` for `Pair`. Without this
        // touch, the union has no reachable use-site and no
        // `Ty::Union { item: Pair, .. }` entry is interned, so
        // Change E's `model.types` walk would not see it.
        unsafe { p.x }
    }
    pub fn classify(s: &State) -> u32 {
        match s {
            State::Idle => 0,
            State::Active(n) => *n,
            State::Error => LIMIT,
        }
    }
    pub fn use_u32(w: &Wrap<u32>) -> u32 { w.v }
    pub fn use_u64(w: &Wrap<u64>) -> u64 { w.v }

    pub trait Reader { fn read(&self) -> u32; }
    impl Reader for Point { fn read(&self) -> u32 { self.x as u32 } }
    pub fn dispatch(r: &dyn Reader) -> u32 { r.read() }
    ```

    The fixture is `#![no_std]` and free of `regseq::*` macros
    so the bridge sees it as plain Rust source (matching the
    other `test-ir-fixtures` shapes). The `dispatch(&dyn
    Reader)` call site forces synthesis of a `VTableType` Item
    (per §13.2) so the corresponding `InstanceKind::Struct`
    coverage required by Change E is exercised end-to-end.

C2. Add a matching integration test
    `crates/regseq-model/tests/e2e_instance_completeness.rs`
    that drives the bridge against the new fixture (mirror the
    helper-invocation shape in
    `crates/regseq-model/tests/e2e_enum_round_trip.rs:34-36`),
    then asserts BOTH validator entry points:
    - `model.validate(&bridge_input)` returns `Ok(())`, where
      `bridge_input` is constructed via the canonical
      `BridgeInput::matching(&model)` helper (or the equivalent
      shape the existing E2E tests use; see
      `crates/regseq-model/tests/id_referential_integrity_validator.rs:15-17`).
      This is the model-correctness check.
    - `model.validate_bridge_completeness()` returns `Ok(())`.
      This is the bridge-completeness check added by Change G.
      `run_bridge` itself invokes this method before returning,
      so the explicit call here is a redundant guard that pins
      the contract at the test surface.
    - `model.instances` contains the following Instance shapes
      keyed by Item kind and ItemKind:
      - one `InstanceKind::Struct` for `Point`, `Pair` is owned
        by Union, plus exactly two for `Wrap<_>` (one per
        monomorphisation per Test Plan A4's cardinality rule).
      - one `InstanceKind::Enum` for `State`.
      - one `InstanceKind::Union` for `Pair`.
      - one `InstanceKind::Static { ty, init: Some(_) }` each
        for `COUNTER` and `FLAG`; `FLAG`'s parent Item carries
        `ItemKind::Static { mutability: Mutability::Mut }`.
      - one `InstanceKind::Const { ty, value }` for `LIMIT`.
      - at least one `InstanceKind::Struct` whose parent Item
        carries `SyntheticItemKind::VTableType { .. }` (the
        synth-vtable coverage required by Change E).
    - An insta snapshot under
      `crates/regseq-model/tests/snapshots/e2e_instance_completeness__render.snap`
      pins the `PrettyPrinter::new(&model).render()` output so
      future regressions surface here.

### D. Workspace regression-clean

D1. `cargo test --workspace` passes after the expected
    snapshot drift on
    `crates/regseq-model/tests/snapshots/e2e_synthesis__vtable_render.snap`
    is re-baselined. The drift is the intended consequence of
    Change E: nominal `InstanceKind::Struct` definitions now
    render at definition-site per `pretty-printer.md`
    §10.2.4. In the vtable E2E fixture
    (`crates/test-ir-fixtures/fixtures/trait_object.rs`), this
    surfaces two new struct definitions in the rendered output
    -- both the synth `VTableType` struct AND the source
    `StaticByte` struct
    (`crates/test-ir-fixtures/fixtures/trait_object.rs:8`) --
    which is the bug fix this change ships. The implementer
    MUST commit the re-baselined snapshot.

    No other snapshot is expected to drift; closure E2E
    (`crates/regseq-model/tests/snapshots/e2e_synthesis__closure_render.snap`),
    diag-log E2E (`crates/regseq-driver/tests/snapshots/...`),
    and the new `e2e_instance_completeness__render.snap` are
    independent of the vtable fixture. If any OTHER snapshot
    drifts, that is a regression to investigate before
    re-baselining.

D2. `cargo fmt --all -- --check` passes.

D3. `cargo clippy --workspace --tests -- -D warnings` passes.

## Review

Once tests pass, request a Codex review of the implementation
by invoking `scripts/python/codex_runner.py` via the Bash tool
(per `SESSION.md` Review section). Use the four-block template
verbatim:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --cwd $(pwd) --effort high --timeout 1500 --message-only
<task>
Review the implementation of regseq-driver-55 against its task
spec at
.harness/tasks/regseq-driver/55-phase5-instance-construction-completeness.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-driver/src/bridge/mod.rs
  (Phase-2 walker extension for MonoItem::Static per Change A;
  hir_crate_items walker for Const per Change B; new
  construct_source_static_instances / construct_source_const_instances /
  construct_nominal_adt_instances helpers per Changes C/D/E;
  Phase-5 orchestrator slot ordering per Change F)
- crates/regseq-driver/src/bridge/ty.rs (source_static_mutability
  helper if extracted; unchanged ItemKind::Struct/Enum/Union
  interning at translate_adt)
- crates/regseq-driver/src/bridge/const_.rs
  (translate_static_init / translate_const_value paths if
  extended for source-static / source-const Instance population)
- crates/regseq-model/src/model.rs
  (new validate_bridge_completeness method as a separate entry
  point per Change G; foreign-linkage and synthetic-Item
  carve-outs)
- crates/regseq-driver/tests/bridge_phase5_instance_completeness.rs
  (new unit tests per Test Plan A1-A9)
- crates/regseq-model/tests/validate_item_instance_coverage.rs
  (new validator tests per Test Plan B1-B3)
- crates/test-ir-fixtures/fixtures/instance_completeness.rs
  (new shared fixture per Test Plan C1)
- crates/regseq-model/tests/e2e_instance_completeness.rs
  (new integration test per Test Plan C2)
- crates/regseq-model/tests/snapshots/e2e_instance_completeness__render.snap
  (new insta snapshot per Test Plan C2)

Focus on:
- Slot ordering: the three new constructors MUST land between
  construct_function_instances and construct_closure_call_instances
  in construct_instances_and_resolve_cross_references per Change F.
  Verify the closure-captures path at mod.rs:1336-1344 is NOT
  duplicated by construct_nominal_adt_instances; the latter MUST
  filter out SyntheticItemKind::ClosureCaptures per Change E step 2.
- VTableType handling: construct_nominal_adt_instances MUST cover
  SyntheticItemKind::VTableType per Change E step 2; verify it does
  NOT collide with synth/vtable.rs::push_vtable_instance_statics
  (which handles VTableInstance, not VTableType).
- Cardinality rule: one Instance per (Item, GenericArgs, flavor)
  per spec.md:1226-1229. Verify the generic Struct test A4 produces
  TWO Instance rows (one per monomorphisation), not one.
- Foreign carve-out: construct_source_static_instances and
  construct_source_const_instances MUST NOT produce Instances for
  Items with linkage == Linkage::Foreign. The validator check at
  Change G MUST also carve out foreign Items.
- Validator placement: `validate_bridge_completeness` is a
  separate public method on `RegseqModel`, NOT folded into
  `RegseqModel::validate(&mut self, &BridgeInput)`. Verify
  `run_bridge` invokes it after the final
  `sort_instances_by_f4_2()` at
  `crates/regseq-driver/src/bridge/mod.rs:635`. Verify the
  partial-model test corpus at
  `crates/regseq-model/tests/{id_referential_integrity_validator,
  layout_validator}.rs` is NOT migrated and continues to call
  `validate(&BridgeInput::matching(model))`.
- Mutability propagation: source static mutability MUST flow from
  rustc tcx.static_mutability to ItemKind::Static { mutability }
  per spec.md §8.0; assert via Test Plan A6.
- AssocConst coverage: Change B's walker MUST include
  DefKind::AssocConst per spec.md:2334-2346 (esp. line 2345)
  alongside DefKind::Const; verify both forms surface as
  ItemKind::Const Items.
- Source-ADT DefId resolution: Change E MUST NOT call
  `source_item_def_id` for ADT Items. The helper at
  bridge/mod.rs:1529-1539 scans `tcx.hir_body_owners()`, which
  covers HIR body owners (functions, statics, consts -- which
  is why Changes C and D may use it) but does NOT include ADT
  definitions (struct/enum/union have no HIR body). The
  model.types walk in Change E is the authoritative
  enumeration; the rustc DefId is not needed at this step.
- Union discovery: the fixture's `touch_pair(&Pair)` function
  MUST force Phase-2 `Ty::Union` discovery so Change E's
  model.types walk reaches the union. Without the touch, no
  Ty::Union entry is interned and no Union Instance can be
  produced.
- Pretty-print stability: the `e2e_instance_completeness` snapshot
  at Test Plan C2 MUST be deterministic across builds (F4 sort +
  F4-2 sort applied).
- No defensive fallbacks: the new passes MUST panic with a clear
  message if their preconditions fail (e.g. Item with kind Struct
  but no matching Ty::Struct entry), NOT return Ok silently per
  the §2 "No defensive fallbacks" rule.
- Test Plan items A1-A9, B1-B3, C1-C2, D1-D3 must all be present
  and exercised.

Do NOT flag anything explicitly marked out of scope (Non-Goals,
foreign-decl handling, spec edits, phase-1b codegen).
</task>

<grounding_rules>
Cite file:line for every claim and every required fix. Do not
speculate about code you did not read. Mark hypotheses as
hypotheses.
</grounding_rules>

<structured_output_contract>
First line: VERDICT: APPROVED | REQUIRES FIXES
If REQUIRES FIXES: numbered list, each entry exactly
  <file>:<line-range> -- <what is wrong> -- <what the spec requires>
If APPROVED: one paragraph (<=120 words) summarizing what was
verified and why any deliberate deviations are acceptable.
Total response <= 500 words.
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, form a verdict,
stop.
</default_follow_through_policy>
EOF
```

Iterate (fix -> re-invoke `codex_runner.py`) until VERDICT:
APPROVED.

## Quality Gate

Workspace-wide because this task spans `regseq-driver` (Phase-5
constructors + Phase-2 walker extension + unit tests),
`regseq-model` (validator extension + validator tests + E2E
integration test), and `test-ir-fixtures` (new shared fixture):

```
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --tests -- -D warnings
```

The authoritative gate is `task.py done`, which runs cleanup
plus workspace-wide test/fmt/clippy per `SESSION.md` Testing
Requirements.

## Acceptance Criteria

- `crates/regseq-driver/src/bridge/mod.rs::translate_with_interleaved_synthesis`
  handles `MonoItem::Static` per Change A and enumerates
  top-level `const` items per Change B.
- Three new Phase-5 helpers exist in
  `crates/regseq-driver/src/bridge/mod.rs`:
  `construct_source_static_instances`,
  `construct_source_const_instances`,
  `construct_nominal_adt_instances`, all slotted between
  `construct_function_instances` and
  `construct_closure_call_instances` per Change F.
- `construct_nominal_adt_instances` covers source
  `Struct`/`Enum`/`Union` Items AND synth `VTableType` Items;
  filters out `SyntheticItemKind::ClosureCaptures` to avoid
  double-pushing.
- For source-defined Items: every reachable monomorphisation of
  `struct/enum/union` produces one `InstanceKind::{Struct, Enum,
  Union} { layout }` Instance; every source `static` produces one
  `InstanceKind::Static { ty, init: Some(_) }` Instance; every
  source `const` produces one `InstanceKind::Const { ty, value }`
  Instance.
- A new public method
  `RegseqModel::validate_bridge_completeness(&self) ->
  Result<(), InvariantViolation>` is added per Change G,
  invoked from `bridge::run_bridge` after the final
  `sort_instances_by_f4_2()` at
  `crates/regseq-driver/src/bridge/mod.rs:635`. The method
  skips synthetic Items (`item_synthetic_kinds[i] !=
  SyntheticItemKind::None`) and foreign Items
  (`linkage == Linkage::Foreign`). The method is
  DELIBERATELY NOT folded into
  `RegseqModel::validate(&mut self, &BridgeInput)`; the
  partial-model validator test corpus stays intact.
- `crates/regseq-driver/tests/bridge_phase5_instance_completeness.rs`
  exercises Test Plan A1-A9.
- `crates/regseq-model/tests/validate_item_instance_coverage.rs`
  exercises Test Plan B1-B3 (negative regression, foreign
  carve-out, source-derived positive path) by invoking
  `model.validate_bridge_completeness()` (the new
  bridge-completeness entry point added by Change G). B1
  additionally cross-checks that
  `model.validate(&BridgeInput::matching(&model))` returns
  `Ok(())` for the SAME partial model, pinning the
  deliberately-distinct entry-point contract.
- `crates/test-ir-fixtures/fixtures/instance_completeness.rs`
  added per Test Plan C1;
  `crates/regseq-model/tests/e2e_instance_completeness.rs`
  asserts the Instance shapes and pins
  `e2e_instance_completeness__render.snap` per Test Plan C2.
- `cargo test --workspace` regression-clean.
- `cargo fmt --all -- --check` and
  `cargo clippy --workspace --tests -- -D warnings` pass.
- Codex Task Review returns VERDICT: APPROVED.
