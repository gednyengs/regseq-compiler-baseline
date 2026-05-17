# regseq-driver-48: Post-DiagLogPeel Dead-Item / Dead-Ty Elimination (`LogArg`-Survival Spec Contract)

## Description

Close the spec.md §26.6 `I-DiagLog-CallArity` carrier-survival
contract gap surfaced by the regseq-driver-45 audit-fix sequence.
After DiagLogPeel (`crates/regseq-driver/src/bridge/diag_log/peel.rs`)
collapses every `LogArg::from(<expr>)` wrapper Call into a `Goto`
to the wrapper's continuation block, the `From`-impl callees become
unreferenced from any surviving terminator -- but the `Item`s, the
`Instance`s, the per-Item `FnSig` entries, and the `Ty::Enum {
item: <LogArg-item> }` entries in the `TyInterner` all survive.
spec.md §26.6 (`spec.md:12705-12707`) pins:

> The `LogArg::from(...)` wrappers, the slice aggregate, and the
> `LogArg` type do NOT appear in RegseqModel.

The §26.6 contract is enforced at validation time by walks that
do NOT honour reachability:
`RegseqModel::validate` checks every stored `Ty` in the
TyInterner and resolves every nominal `ItemId`
(`crates/regseq-model/src/model.rs:1541-1580`). The
regseq-model-94 Test Plan D7/D8 explicitly walks the entire
TyInterner and every Item in `model.items`. So discharging the
contract requires more than nulling Body local-decls or removing
top-level Items: every stale TyInterner / FnSig interner entry
referencing the removed Items must also be pruned, and every
surviving `TyId` / `FnSigId` reference must be remapped to the
new dense ID range.

This task lands a focused dead-code elimination pass after
DiagLogPeel that computes the transitive closure of DiagLog
residue across four interner / table surfaces (Items, Instances,
TyInterner, FnSig interner), prunes the dead entries, and
remaps every surviving reference. Scope is intentionally
**residue-driven**, not "generalised DCE over the whole model":
removal seeds are the wrapper Instances DiagLogPeel collapsed
plus the LogArg Item; transitive removal proceeds only along
chains that anchor at those seeds. Removal of any other
unreferenced entry (orphan closures, unused drop-glue, dead
synthesised statics) is explicitly out of scope and stays as
the deferred §26.10 contract.

### Audit-derived amendment (spec_version 2, 2026-05-17)

The initial spec scoped removal to "the wrapper Instances
DiagLogPeel collapsed plus the LogArg Item (when no surviving
Ty references it)". That contract is **unsatisfiable for any
real fixture** because rustc materialises every impl method in
an rlib crate type, not only the ones an author call site
invokes. For the canonical `diag_logging.rs` fixture (which
uses only `From<u32>` calls), the `<LogArg as From<&'a str>>::from`
impl is still emitted as a Function Instance whose body's
return-place `LocalDecl` pins LogArg via the type interner.
Strict residue-driven scope keeps that sibling alive, which
keeps LogArg alive, which makes spec.md §26.6's "the `LogArg`
type does NOT appear in RegseqModel" clause unsatisfiable.

Plus three subordinate contract gaps the original spec did not
account for:

- **Log-intrinsic FnSig pinning.** The four `log_<sev>`
  intrinsic Items have FnSig `(fmt: &'static str,
  &[LogArg<'_>]) -> ()`. The `&[LogArg<'_>]` input is a
  load-bearing reference to LogArg via the type interner even
  after every wrapper Call's runtime args are peeled. Removing
  LogArg requires either removing the log_<sev> Items
  themselves (NOT desirable -- they remain the
  diagnostic-logging emit points and carry the
  `IntrinsicKind::Log(_)` AttrData) or rewriting their FnSigs
  to drop the `&[LogArg<'_>]` input. The latter is the
  pragmatic shape that preserves §26.3 + §26.6 intent.
- **HandleTable "MUST not remove handle-bound Items" rule.**
  Phase 4 binds *every* Item via `HandleTable::bind_final_item_id`
  at `crates/regseq-driver/src/bridge/assign.rs:33`, not just
  bridge-synthesised entries. The original Change B step 6
  rule ("panic if any still-bound handle maps to a removed
  Item") is therefore unsatisfiable: every removable Item is
  handle-bound. The contract is preserved in spirit by the
  pass running LAST in Phase 5 (no post-Phase-5 consumer
  calls `handles.resolve`), with handle bindings for removed
  Items dropped and the dense ItemId remap applied.
- **`model.validate()` post-elim.** The intrinsic FnSig
  rewrite creates a per-Call mismatch between the surviving
  log Call's `args` (e.g., `u32` post-DiagLogPeel) and the
  rewritten `log_<sev>` `FnSig.inputs` (now just `&str fmt`).
  The model validator's per-Call arg/input type-coherence
  check at `crates/regseq-model/src/model.rs` flags this.
  spec.md §23.6.2 B-4 / B-6 already pin an `I-DiagLog-CallArity`
  exception for Log-intrinsic Calls' arity vs. signature
  mismatch (`spec.md:9650+` / `spec.md:11792+`), but the
  validator does NOT currently honour the exception. The fix
  belongs in a sibling task (proposed `regseq-driver-49`,
  validator B-4/B-6 §26.6 exception); this task's integration
  test temporarily skips `model.validate()` and locks the
  structural §26.6 contract via direct walks instead.

This amendment formalises four contract relaxations and one
contract addition, and acknowledges the deferred `validate()`
piece. The full set: **Change F** (widened seed predicate),
**Change G** (step-0 log-intrinsic FnSig rewrite),
**revised Change B step 6** (HandleTable drop+remap pattern
instead of panic), **revised Test Plan item 9** (sibling
From-impl-for-LogArg removed, not surviving), **revised Test
Plan item 10** (`validate()` deferred to regseq-driver-49).

## Input Context

- `docs/discussions/regseq-model/spec.md` §25.7.3 (DiagLogPeel
  body-level pass; `spec.md:11772-12100`)
- `docs/discussions/regseq-model/spec.md` §26.4 (LogArg
  argument carrier + carrier-survival rule;
  `spec.md:12481-12638`)
- `docs/discussions/regseq-model/spec.md` §26.6 (Bridge
  translation contract + `I-DiagLog-CallArity`;
  `spec.md:12692-12762`) -- especially the load-bearing
  "the `LogArg` type do NOT appear in RegseqModel" clause at
  `spec.md:12705-12707`
- `docs/discussions/regseq-model/spec.md` §26.10 (deferrals and
  scope boundaries; `spec.md:12909+`)
- `crates/regseq-model/src/model.rs::validate_ty_interner`
  (`model.rs:1541-1580`) -- the validation walk this task's
  contract must satisfy
- `crates/regseq-model/src/model.rs::sort_instances_by_f4_2`
  (`model.rs:933-1044`) -- the InstanceId remap reference
  implementation
- `crates/regseq-model/src/model.rs::sort_items_by_f4` (lands
  in `regseq-driver-47`) + the shared
  `for_each_item_id_mut` walker (also `regseq-driver-47`,
  Change A.1) -- the ItemId-remap surface this task composes
  with
- `crates/regseq-model/src/ty.rs` -- Ty variants that embed
  `TyId` references (`Ty::Array(TyId, _)`, `Ty::Slice(TyId)`,
  `Ty::Ref(TyId, _)`, `Ty::Ptr(TyId, _)`,
  `Ty::Tuple { fields: Vec<TyId>, .. }`,
  `Ty::Dynamic { vtable_ty: TyId, .. }`,
  `Ty::Enum/Struct/Union { args: GenericArgs }` where
  `GenericArg::Type(TyId)`)
- `crates/regseq-model/src/ty_interner.rs` (`TyInterner` API:
  `intern`, `resolve`, `iter` at `ty_interner.rs:88-141`)
- `crates/regseq-model/src/fn_sig.rs` -- FnSig shape
  (`inputs: Vec<TyId>`, `output: TyId`) and the FnSig interner
- `crates/regseq-model/src/args_key.rs::GenericArg` (the
  `Type(TyId)` variant)
- `crates/regseq-model/src/body.rs:112-114` -- `LocalDecl.ty: TyId`
- `crates/regseq-model/src/statement.rs` and
  `crates/regseq-model/src/rvalue.rs` -- statement / Rvalue
  surfaces carrying `TyId` (Cast TyId, AggregateKind::Array
  TyId, etc.)
- `crates/regseq-model/src/const_value.rs:18-50` -- Const
  variants carrying `TyId` (`Const::NullPtr(TyId)`, any
  aggregate Ty references)
- `crates/regseq-model/src/instance.rs` -- `InstanceKind`
  variants carrying `TyId` (`FunctionInstanceData.fn_sig:
  FnSigId`; closure-capture struct Ty fields)
- `crates/regseq-driver/src/bridge/diag_log/peel.rs`
  (`diag_log_peel_pass`, `peel_body_against_model`,
  `peel_log_arg_from_wrapper`, `callee_is_from_logarg`,
  `collapse_wrapper_calls`, `null_carrier_local_decl_types` --
  the existing pass surface this task extends)
- `crates/regseq-driver/src/bridge/mod.rs`
  (`construct_instances_and_resolve_cross_references` at
  `mod.rs:990-1013` -- the Phase-5 envelope this task slots
  into) and `crates/regseq-driver/src/bridge/handles.rs:22-55`
  (`HandleTable` + the `remap_item_ids` method from
  `regseq-driver-47`)
- `crates/regseq-driver/src/bridge/body/materialise.rs`
  (`run_phase5_body_passes` -- the body-pass ordering this
  task extends)
- `.harness/tasks/regseq-driver/45-diag-log-peel-and-format-grammar.md`
  (the recognition + peel pass this task extends)
- `.harness/tasks/regseq-driver/47-f4-late-item-reordering-and-id-remap.md`
  (prerequisite: F4-1 sort + ItemId remap + the shared
  `for_each_item_id_mut` walker must land first)
- `.harness/tasks/regseq-model/94-e2e-diagnostic-logging.md`
  (the downstream task this fix unblocks; Test Plan D7/D8 at
  lines 261-274 is the contract this task's pruning makes
  satisfiable)
- `.harness/spec-batches/regseqmodel-phase-1a.md` (parent
  brief; this task is a phase-1a bridge follow-up addition
  beyond the original BR-01..BR-08b table)

## Implementation Notes

Five coupled changes. The DCE fixed-point computation (Change B)
is the load-bearing change; the residue accumulator (Change A),
the model-side helpers (Change C), the shared walkers
(Change D), and the pipeline wiring (Change E) are the
surrounding contract.

### Change A: DiagLogPeel tracks collapsed-wrapper residue

In `crates/regseq-driver/src/bridge/diag_log/peel.rs`, extend
the wrapper-collapse path to also capture the wrapper Call's
callee `InstanceId` BEFORE
`collapse_wrapper_calls` rewrites the terminator. The
existing `peel_log_arg_from_wrapper`
(`peel.rs::peel_log_arg_from_wrapper`) already returns the
wrapper `BlockId`; extend its return shape to also return the
`CallCallee::Direct(instance_id)` so the caller can push the
`InstanceId` onto a bridge-side accumulator.

Add to `BridgeCtxt`
(`crates/regseq-driver/src/bridge/mod.rs`):

```rust
/// Bridge-side accumulator for DiagLogPeel residue. Populated
/// during `peel_body_against_model` per collapsed wrapper Call;
/// drained by `eliminate_diag_log_peel_residue` after every
/// Body has been peeled (Phase-5 sub-pass order in Change E).
pub struct DiagLogPeelResidue {
    /// InstanceIds of wrapper Calls collapsed to Goto. Each is
    /// the seed for transitive dead-set computation in
    /// `eliminate_diag_log_peel_residue`.
    pub collapsed_wrapper_instances: Vec<InstanceId>,
}
```

Wire the accumulator through `BridgeCtxt` so it survives
across body-pass invocations and Phase-5 sub-pass boundaries.

### Change B: New Phase-5 sub-pass: `eliminate_diag_log_peel_residue`

New file
`crates/regseq-driver/src/bridge/diag_log/dead_item_elim.rs`
with:

```rust
//! Post-DiagLogPeel dead-Item / dead-Instance / dead-Ty /
//! dead-FnSig elimination per spec.md section 26.6 ("the
//! LogArg type does NOT appear in RegseqModel"). Runs after
//! every Body's DiagLogPeel completes so the residue
//! accumulator carries the full collapsed-wrapper set.
//!
//! Scope is residue-driven, not generalised DCE: removal
//! seeds are the wrapper Instances DiagLogPeel collapsed plus
//! the LogArg Item; transitive removal proceeds only along
//! chains that anchor at those seeds. Unrelated unreferenced
//! entries (orphan closures, unused drop-glue, dead
//! synthesised statics) are explicitly NOT touched.

pub fn eliminate_diag_log_peel_residue(
    cx: &mut BridgeCtxt<'_>,
) -> Result<(), BridgeError> { /* ... */ }
```

Implementation, in dependency order (downstream depends on
upstream's keep-set being computed first):

0. **Step-0 log-intrinsic FnSig rewrite** (audit-derived
   amendment). Gated on the residue accumulator being
   non-empty (idempotent no-op when DiagLogPeel collapsed
   nothing in any body; preserves Step 1's strict
   residue-driven no-op invariant for empty-`&[]` recognition
   / peel integration tests). When the gate holds, rewrite
   every `log_<sev>` intrinsic Function Instance's FnSig per
   the contract introduced above. Implementation function:
   `rewrite_log_intrinsic_signatures(cx) -> HashSet<FnSigId>`
   in `crates/regseq-driver/src/bridge/diag_log/dead_item_elim.rs`.
   For each `instance_is_log_intrinsic`-matching Function
   Instance whose current FnSig has a LogArg-bearing input
   (i.e., any input Ty whose resolution touches a LogArg
   ADT, walked transitively through `Ty::Ref`, `Ty::Slice`,
   `Ty::Array`, `Ty::Ptr`, `Ty::Tuple.fields`, etc.):
   - Build a new FnSig with `inputs = vec![original_fmt_input]`
     (the `&'static str` fmt operand at position 0 only),
     `output = unit_ty`, ABI / variadic / safety preserved.
   - Intern the new FnSig; record the old FnSigId in the
     `orphaned_log_intrinsic_fn_sigs` set so the dead-FnSig
     computation in step 3 picks it up.
   - Pad `sig_c_abi_lowerings` to `sigs.len()` with `None`.
   - Update the Instance: `data.fn_sig = new_sig_id`;
     `data.body = None` (the intrinsic body is a stub
     `unimplemented!()` per spec.md §26.3 anyway, and the
     C backend lowering at §26.9 owns body synthesis per
     the deferred phase-1b contract).

   This step is idempotent: any Instance whose current FnSig
   already has no LogArg-bearing inputs is skipped (re-runs
   are no-ops). Runs FIRST so the dead-FnSig + dead-TyId
   walks (steps 3 + 4) see the rewritten FnSigs as the
   "surviving" reference set.

   Rationale: the LogArg ADT cannot be removed while any
   surviving FnSig's inputs / outputs resolve to a LogArg-
   bearing Ty. The log_<sev> intrinsic FnSigs are the only
   surviving pinning references after the widening seed
   admits all From-impl Instances; rewriting them is the
   final structural step that makes the §26.6 contract
   satisfiable.

1. **Seed dead-Instance set** in two phases per the audit-
   derived amendment above:

   1.1. **Primary seeds** from
   `cx.diag_log_peel_residue.collapsed_wrapper_instances`,
   filtered through the structural-recognition predicate
   (`is_from_logarg_instance`; lifted to a `pub(crate)`
   helper in `diag_log/mod.rs` and shared with
   `peel.rs::callee_is_from_logarg`). The predicate confirms
   `instance.item`'s def_path ends with `::From::from` AND
   `generic_args[0]` resolves to a nominal ADT whose Item
   def_path terminal segment equals `"LogArg"`.

   1.2. **Widening (anchored at the primary seeds)** -- iff
   at least one primary seed was added in step 1.1, walk
   every Function Instance in `model.instances` and admit
   any that satisfies the structural LogArg-From-impl
   predicate to the dead set. The widening is REQUIRED for
   the §26.6 contract on real fixtures (rustc materialises
   every impl method in an rlib; sibling From-impl Instances
   are emitted whether or not their wrappers are collapsed).
   Anchoring at "primary seeds non-empty" preserves the
   residue-driven invariant: the pass is a strict no-op when
   no wrapper was collapsed; it only widens once DiagLogPeel
   has confirmed at least one LogArg-related residue exists.

   Both trait-side recognition (`is_from_logarg_instance`,
   matching the bridge's `<T as Trait>::method` interning
   shape) AND impl-side recognition (`instance_is_logarg_impl_from`,
   matching the post-monomorphisation impl-method def_path
   shape) must run -- the bridge may produce either form
   depending on the rustc resolution path.

2. **Seed dead-Item set** from the From-impl Items whose
   only surviving Instances are all in the dead-Instance
   set. Walk `model.instances`; build a multiset of
   `instance.item` references; for each Item, if all its
   referencing Instances are dead, mark it dead. The From-
   impl Items satisfy this (their only Instance was the
   wrapper).

3. **Compute referenced-FnSig set** by walking every
   surviving (NOT dead) Instance's `InstanceKind::Function.fn_sig`
   and any other surviving structure that holds `FnSigId`.
   Any `FnSigId` not in this set, AND that was referenced
   from at least one DEAD Item / Instance, is dead. (The
   "and was referenced from dead" qualifier preserves the
   residue-driven scope: orphan FnSigs unrelated to DiagLog
   stay alive.)

4. **Compute referenced-TyId set** by walking every
   surviving Item, every surviving Instance, every surviving
   FnSig, every surviving Body, every surviving Const. Any
   `TyId` not in this set, AND that was referenced from at
   least one dead entry (Item / Instance / FnSig), is dead.
   Same residue-driven qualifier.

5. **Augment dead-Item set with the LogArg ADT Item** iff:
   - It exists (find by structural search:
     `model.items.iter().find(|item| item.def_path.segments
     .last().map(|seg| segment_name(seg) == "LogArg") ==
     Some(true))`).
   - It has no surviving Ty reference (i.e., every Ty in the
     surviving Ty set that resolves to
     `Ty::Enum { item: <LogArg-id>, .. }` was already in the
     dead-TyId set from step 4).
   - It has no surviving Instance reference. The LogArg ADT
     Item itself typically has no Instance (ADTs don't get
     Instances in the same sense functions do), but check
     defensively.

6. **Removal pass, in dependency order:**
   - `model.remove_instances_and_remap(&instance_keep)` first
     (instance removal does not affect Item / Ty / FnSig
     indices).
   - `model.remove_fn_sigs_and_remap(&fn_sig_keep)` next.
   - `model.remove_tys_and_remap(&ty_keep)` next.
   - `model.remove_items_and_remap(&item_keep)` last (Item
     removal is the load-bearing remap and uses the shared
     `for_each_item_id_mut` walker).
   - **HandleTable drop + dense-remap pattern** (audit-
     derived amendment): Phase 4 binds *every* Item via
     `HandleTable::bind_final_item_id` (`assign.rs:33`), so
     the prior spec rule "MUST not remove handle-bound
     Items" is unsatisfiable -- every removable Item is
     handle-bound. The contract is preserved in spirit by
     the pass running LAST in Phase 5 (no post-Phase-5
     consumer calls `handles.resolve`); operationally:
     (a) walk `cx.handles.handle_to_item_id` and DROP every
     binding whose target ItemId index appears as `None`
     in the returned `Vec<Option<ItemId>>` (the slot was
     removed), then (b) build a dense `Vec<ItemId>` from
     the remap by mapping `None` slots to `ItemId::ZERO`
     (sentinel for "removed; unreachable after step (a)"),
     and (c) call `cx.handles.remap_item_ids(&dense_remap)`
     for forward safety. Step (a) ensures no surviving
     handle binding resolves to an unrelated post-removal
     Item; step (b/c) updates surviving bindings through
     `regseq-driver-47`'s HandleTable contract.

7. **Drain the residue accumulator** so the pass is
   idempotent (re-running it is a no-op).

### Change C: Model-side removal helpers

In `crates/regseq-model/src/model.rs`, alongside
`sort_instances_by_f4_2` and the new `sort_items_by_f4` from
`regseq-driver-47`, add four new methods:

```rust
/// Remove items at every index where `item_keep[idx] == false`,
/// compacting `model.items` and every IndexVec<ItemId, _> side
/// table, and remap every surviving ItemId reference everywhere
/// it appears in the model via the shared
/// `for_each_item_id_mut` walker (landed by regseq-driver-47).
/// Returns the old-to-new ItemId remap as `Vec<Option<ItemId>>`
/// (None = removed).
///
/// Removed Items must have NO surviving references in the
/// model. Caller responsibility; this method does NOT verify.
pub fn remove_items_and_remap(&mut self, item_keep: &[bool]) -> Vec<Option<ItemId>>;

/// Same shape, for Instances. Walks
/// `body.basic_blocks[*].terminator` CallCallee::Direct
/// (`crates/regseq-model/src/terminator.rs:113-115`).
pub fn remove_instances_and_remap(&mut self, instance_keep: &[bool]) -> Vec<Option<InstanceId>>;

/// Remove Ty entries at every TyId where `ty_keep[idx] == false`,
/// compacting the `TyInterner`'s internal slab, and remap every
/// surviving TyId reference everywhere it appears in the model
/// via the new `for_each_ty_id_mut` walker (Change D below).
/// Returns the old-to-new TyId remap as `Vec<Option<TyId>>`.
///
/// In addition to walking the `for_each_ty_id_mut` surface,
/// this method REBUILDS the `model.closure_captures_struct_ty`
/// HashMap with remapped keys + values: HashMap key mutation
/// requires re-insertion (`crates/regseq-model/src/model.rs:103`).
/// Even when no LogArg TyId is itself a key, TyInterner
/// compaction shifts every surviving TyId, so the keys (and
/// values) must be rewritten. Drain the HashMap, remap each
/// `(key_ty, value_ty)` pair through the remap table, and
/// re-insert (skipping any pair where either side was
/// removed -- which must NOT happen for non-residue keys per
/// the residue-driven contract; if it does, panic in debug
/// builds).
///
/// Removed Ty entries must have NO surviving references.
/// Content-keyed dedup is NOT consulted for removal -- the
/// caller's keep-set is authoritative. (Two TyIds resolving to
/// the same Ty content cannot exist in the interner, so the
/// keep-set is implicitly per-content.)
pub fn remove_tys_and_remap(&mut self, ty_keep: &[bool]) -> Vec<Option<TyId>>;

/// Remove FnSig entries at every FnSigId where
/// `fn_sig_keep[idx] == false`, compacting both the FnSig
/// interner AND `model.sig_c_abi_lowerings` in lock-step
/// (the latter is a `IndexVec<FnSigId, Option<CAbiLowering>>`
/// parallel side table per `model.rs:84-85`; validation
/// requires `sig_c_abi_lowerings.len() == sigs.len()` per
/// `layout_validator.rs:298-308`). Remap every surviving
/// FnSigId reference via the new `for_each_fn_sig_id_mut`
/// walker (Change D below). Returns the old-to-new FnSigId
/// remap as `Vec<Option<FnSigId>>`.
pub fn remove_fn_sigs_and_remap(&mut self, fn_sig_keep: &[bool]) -> Vec<Option<FnSigId>>;
```

### Change D: Shared TyId + FnSigId walkers

In `crates/regseq-model/src/model.rs`, alongside the
`for_each_item_id_mut` walker that `regseq-driver-47` lands
(Change A.1 in that task), add two sister walkers:

```rust
/// Apply `f` to every TyId reference in the model. Visits:
/// - Type-interner entries: every `Ty` variant that embeds
///   `TyId` -- `Array(TyId, _)`, `Slice(TyId)`,
///   `Ref(TyId, _)`, `Ptr(TyId, _)`,
///   `Dynamic { vtable_ty, .. }`, `Tuple { fields, .. }`
///   per `crates/regseq-model/src/ty.rs:242-310` -- AND
///   every `GenericArg::Type(TyId)`
///   (`crates/regseq-model/src/args_key.rs:30-35`) inside
///   ANY `Ty` variant that carries `GenericArgs`: `Enum`,
///   `Struct`, `Union`, `FnDef(ItemId, GenericArgs)`
///   (`ty.rs:255`), `Closure(ItemId, GenericArgs)`
///   (`ty.rs:271`).
/// - Inline `Layout` TyId fields reachable from Ty:
///   `FieldLayout.ty`
///   (`crates/regseq-model/src/layout.rs:58-62`) and
///   `TagScalar.scalar_ty`
///   (`crates/regseq-model/src/layout.rs:146-154`). These
///   are validated by
///   `crates/regseq-model/src/model.rs::validate_layouts`
///   at `model.rs:1587-1643`; missing them yields stale
///   ItemId/TyId references in inline layouts.
/// - FnSig entries: `inputs[*]`, `output`
///   (`crates/regseq-model/src/fn_sig.rs`).
/// - `sig_c_abi_lowerings[*]`: every
///   `Some(CAbiLowering)`'s per-position
///   `CAbiShape::Substituted { emit_as: TyId, .. }`
///   (`crates/regseq-model/src/cabi.rs:13-17`).
/// - Body local_decls: `local_decls[*].ty`
///   (`crates/regseq-model/src/body.rs:112-114`).
/// - Body statements + terminators: every `Rvalue::Cast`
///   Ty, every `Rvalue::Aggregate(AggregateKind::Array(TyId),
///   _)` Ty, any other Rvalue variant carrying TyId.
/// - `PlaceElem::Field(_, TyId)`
///   (`crates/regseq-model/src/place.rs:77-84`) reachable
///   from every Place in every body's statements and
///   terminators (and every Place's projection).
/// - Const interner: `Const::NullPtr(TyId)`, any other
///   Const variant carrying TyId.
/// - Instances: any `InstanceKind` field carrying TyId
///   (closure-capture struct Ty, vtable Ty, etc.) AND
///   every `Instance.generic_args[*]::Type(TyId)`
///   (`crates/regseq-model/src/instance.rs:137-138`;
///   validated at `crates/regseq-model/src/model.rs:2637-2645`).
/// - Model HashMap `closure_captures_struct_ty`
///   (`HashMap<TyId, TyId>` per `model.rs:103`): NOT visited
///   through this walker (HashMap key mutation requires
///   rebuild). The model-side `remove_tys_and_remap` rebuilds
///   this HashMap explicitly per Change C's contract.
pub(crate) fn for_each_ty_id_mut<F: FnMut(&mut TyId)>(&mut self, f: F);

/// Apply `f` to every FnSigId reference in the model. Visits:
/// - Type-interner: `Ty::FnPtr(FnSigId)`
///   (`crates/regseq-model/src/ty.rs:258`). Validated by
///   `validate_ty_interner` at `model.rs:1566`.
/// - Every `InstanceKind::Function.fn_sig`.
/// - Any other `FnSigId` field on `InstanceKind` variants.
/// - `sig_c_abi_lowerings` is positional, not remapped;
///   this walker does NOT visit its FnSigId indexer
///   (caller permutes the IndexVec in lock-step with FnSig
///   removal per Change C).
pub(crate) fn for_each_fn_sig_id_mut<F: FnMut(&mut FnSigId)>(&mut self, f: F);
```

The walkers enumerate the same surfaces that
`remove_tys_and_remap` / `remove_fn_sigs_and_remap` need to
update. Lifting to shared helpers is what makes the reference
surface single-source-of-truth (mirrors
`regseq-driver-47`'s `for_each_item_id_mut` pattern).

### Change E: Bridge pipeline wiring

In `crates/regseq-driver/src/bridge/mod.rs::construct_instances_and_resolve_cross_references`
(`mod.rs:990-1013`), add the new pass LAST in the Phase-5
sub-pass order, AFTER the
`regseq-driver-47` `sort_items_by_f4` step and AFTER
`const_::construct_const_memory_static_instances`:

Updated Phase-5 sub-pass order:

1. `synth::rekey_synthesis_in_phase5`
2. `construct_function_instances`
3. `body::run_phase5_body_passes` (DiagLogPeel fires here)
4. Late-pending drain + `model.sort_items_by_f4()` +
   `cx.handles.remap_item_ids(&item_remap)` (from
   `regseq-driver-47`)
5. `const_::construct_const_memory_static_instances`
6. **`diag_log::dead_item_elim::eliminate_diag_log_peel_residue`
   (new from this task)**

The elimination runs LAST in Phase 5 because:
- It needs every body to have been peeled (so the residue
  accumulator is complete).
- It needs every late-pending Item assigned (so the dead set
  doesn't accidentally include items that arrive late and are
  legitimately referenced).
- It needs every ConstMemoryStatic Instance constructed (so
  the surviving-Instance walk covers the full set).
- The final `model.sort_instances_by_f4_2()` call in
  `run_bridge` (`mod.rs:599`) re-sorts surviving instances
  after this task's removal -- the existing call point is
  preserved.

### Conventions

- Doc comments on every new public item (per `SESSION.md`
  Coding Conventions).
- No unicode symbols in source / docs.
- The structural From-LogArg recognition is lifted to
  `pub(crate)` in `diag_log/mod.rs` for shared use by
  `peel.rs::peel_log_arg_from_wrapper` and
  `dead_item_elim.rs`. Prefer one source of truth.
- All assertions cite the `spec.md` section they verify in a
  one-line comment so the Codex review can cross-reference.

## Non-Goals

- **Generalised dead-code elimination over the model.** This
  task's scope is the residue DiagLogPeel produces plus the
  LogArg ADT Item plus the transitively-unreferenced support
  entries (From-impl Items, their FnSigs, the LogArg Tys).
  Other dead-code (orphan synthesised vtables, unused drop-glue,
  dead synthesised statics, abandoned closures) is out of
  scope; the §26.10 deferral covers those cases.
- **No generalised HandleTable contract change.** This task
  reuses `regseq-driver-47`'s
  `HandleTable::remap_item_ids` on the dense remap derived
  from the removal pass. Out of scope: changing
  `HandleTable::resolve` to return `Option<ItemId>` or
  introducing a removal-aware handle API. The bridge invariant
  -- by the time this task's pass runs, no live handle was
  bound to an Item that this task removes -- holds (the
  ConstMemoryStatic Items are bound by
  `construct_const_memory_static_instances`, which runs
  earlier in the pipeline, and those Items are not part of
  the DiagLogPeel residue).
- **The `IntrinsicKind::Log(_)` `AttrData`** on the four
  `log_<sev>` callee Items. Those Items are NOT removed --
  they are the diagnostic-logging emit points and remain
  referenced by surviving log Calls. The attribute is
  preserved per `spec.md` §3.6.1.
- **Modification of `regseq-driver-45`'s recognition or peel
  contract.** That task's load-bearing changes (carrier
  walker reborrow, LocalDecl ty cleanup, structural From-
  recognition) stay verbatim; this task additively wires a
  new pass that consumes peel's output.
- **F4 ordering after the post-removal model.** Compaction
  preserves relative order among surviving entries; the
  `sort_instances_by_f4_2` call at `mod.rs:599` re-sorts
  surviving Instances post-removal. Items remain in their
  pre-removal F4 order (already established by
  `regseq-driver-47`'s `sort_items_by_f4` earlier in Phase
  5); this task does NOT re-sort items.
- **`closure_captures_struct_ty` HashMap removal of dead
  TyId entries.** This task does NOT delete entries from
  the HashMap as a side-effect of TyId removal; the residue-
  driven contract says no LogArg TyId is a closure-capture
  key (closure-capture TyIds belong to closure synthesis,
  not DiagLog), so no entry should be deletion-eligible.
  However, `remove_tys_and_remap` DOES rebuild the HashMap
  with remapped keys + values (per Change C) because
  TyInterner compaction shifts every surviving TyId
  regardless of LogArg. The rebuild is a remap-only
  transform: if any entry's key OR value resolves to a
  REMOVED TyId, that signals a residue-driven-contract
  violation -- panic in debug builds rather than silently
  drop the entry.
- **`model.validate()` post-elim compliance** (audit-derived
  amendment). The step-0 log-intrinsic FnSig rewrite creates
  a per-Call arg-vs-signature input-type mismatch that the
  current validator flags but `spec.md` §23.6.2 B-4 / B-6
  exception permits (`spec.md:9650+` / `spec.md:11792+`).
  Updating the validator to honour the exception is **NOT**
  in scope for this task; it lands in the proposed follow-
  up `regseq-driver-49` task. This task's integration test
  asserts the structural §26.6 contract via direct walks
  and explicitly skips `model.validate()`. regseq-model-94
  reset and the validate() restoration also defer to
  regseq-driver-49.

## Test Plan

### Unit tests (`crates/regseq-model/src/model.rs::mod tests` + `crates/regseq-driver/src/bridge/diag_log/dead_item_elim.rs::mod tests`)

1. **`remove_items_and_remap` -- middle-item removal.**
   Construct a model with three items + an Instance referencing
   item 2. Mark item 1 (middle) for removal via
   `item_keep = [true, false, true]`. Call. Assert:
   - `model.items.len() == 2`.
   - The Instance.item now references the new index of item 2
     (post-compaction position 1).
   - Every IndexVec<ItemId, _> side table compacted in lock-step.
   - The returned remap is `[Some(0), None, Some(1)]`.

2. **`remove_instances_and_remap` -- removal with surviving
   Call terminator remap.** Construct a model with three
   Instances and a Body whose terminator is
   `CallCallee::Direct(InstanceId(2))`. Remove Instance 1.
   Assert the body's callee InstanceId is updated to the new
   index 1 (post-compaction).

3. **`remove_tys_and_remap` -- TyId reference remap +
   `closure_captures_struct_ty` HashMap rebuild.** Construct
   a model with:
   - Three Tys (Ty 0 a struct, Ty 1 a slated-for-removal
     marker, Ty 2 a struct).
   - A LocalDecl referencing Ty 2.
   - A Ty::Ref(Ty 2, _) entry in the interner.
   - A `closure_captures_struct_ty.insert(Ty 0, Ty 2)`
     entry (neither side scheduled for removal).
   Remove Ty 1. Call. Assert:
   - LocalDecl.ty is now the new index of Ty 2.
   - The Ty::Ref's inner TyId is the new index of Ty 2.
   - The TyInterner's `iter()` yields the surviving Tys in
     insertion-order minus the removed.
   - `closure_captures_struct_ty` now contains exactly one
     entry `(new_idx_of_Ty_0, new_idx_of_Ty_2)`; the old
     `(Ty 0, Ty 2)` key is gone.

4. **`remove_fn_sigs_and_remap` -- FnSig reference remap +
   `sig_c_abi_lowerings` lock-step compaction.** Construct
   a model with:
   - Two FnSigs + `sig_c_abi_lowerings` populated for both
     (`Some(CAbiLowering)` entries; per
     `crates/regseq-model/src/model.rs:84-85` and the
     `len() == sigs.len()` invariant at
     `layout_validator.rs:298-308`).
   - A Function Instance referencing FnSig 1.
   - A `Ty::FnPtr(FnSig 1)` entry in the TyInterner (per
     `crates/regseq-model/src/ty.rs:258`).
   Remove FnSig 0. Assert:
   - The Instance's `fn_sig` is the new index of FnSig 1
     (post-compaction position 0).
   - `Ty::FnPtr`'s inner FnSigId is the new index of FnSig 1.
   - `sig_c_abi_lowerings.len() == sigs.len() == 1` after
     compaction; the surviving entry is the one originally
     at position 1.

5. **`for_each_ty_id_mut` walker coverage.** Build a model
   exercising every TyId-carrying surface enumerated in
   Change D:
   - Ty variants: Array, Slice, Ref, Ptr, Dynamic.vtable_ty,
     Tuple.fields, Enum/Struct/Union.args nested
     GenericArg::Type.
   - Inline Layout TyIds: FieldLayout.ty
     (`crates/regseq-model/src/layout.rs:58-62`) and
     TagScalar.scalar_ty (`layout.rs:146-154`) inside a
     nominal Ty's resolved layout.
   - FnSig.inputs / FnSig.output.
   - sig_c_abi_lowerings: a CAbiShape::Substituted entry
     with a non-zero emit_as TyId (`cabi.rs:13-17`).
   - LocalDecl.ty inside a Body.
   - Body Rvalue surfaces: Rvalue::Cast TyId, and EVERY
     `AggregateKind` variant carrying TyId per
     `crates/regseq-model/src/rvalue.rs:185-223`: `Array`,
     `Tuple`, `Struct`, `Enum`, `Union`, `Closure`,
     `RawPtr` (seven kinds, each carrying one TyId).
   - PlaceElem::Field(_, TyId) inside a Place projection
     (`place.rs:77-84`).
   - Const::NullPtr(TyId)
     (`crates/regseq-model/src/const_value.rs:18-50`).
   - `Instance.generic_args[*]::Type(TyId)`
     (`crates/regseq-model/src/instance.rs:137-138`).

   Apply `for_each_ty_id_mut` with an incrementing-counter
   closure; assert the count equals the expected total. Each
   call site is enumerated above; the test must touch every
   one to lock the walker contract.

6. **`for_each_fn_sig_id_mut` walker coverage.** Build a model
   with:
   - Two Function Instances (asserts each
     `InstanceKind::Function.fn_sig` visited exactly once).
   - One `Ty::FnPtr(FnSigId)` entry in the TyInterner
     (`crates/regseq-model/src/ty.rs:258`; asserts the
     TyInterner-side FnSigId reference is visited).
   Apply `for_each_fn_sig_id_mut` with an incrementing-
   counter closure; assert the total is 3.

7. **Empty removal (no-op).** Pass `keep` slices of all
   `true`. Assert `model.items` / `model.instances` /
   TyInterner / FnSig interner unchanged and every remap is
   identity (`Some(N::from_u32(i))` for every `i`).

8. **`eliminate_diag_log_peel_residue` -- hand-constructed
   residue.** Build a model with a DiagLogPeel-style residue
   (a wrapper Instance whose Item def_path matches
   `<LogArg as From<u32>>::from` + generic_args[0] resolves
   to a LogArg-named Enum + a FnSig referencing the LogArg
   Ty). Populate the residue accumulator with that
   InstanceId. Call the pass. Assert:
   - The wrapper Instance is removed.
   - The From-impl Item is removed (no other Instance refs).
   - The FnSig is removed (only referenced from the dead
     Item).
   - The LogArg Ty entry is removed.
   - The LogArg ADT Item is removed (no surviving Ty refs
     remain).

9. **`eliminate_diag_log_peel_residue` -- sibling From-impl-
   for-LogArg removal (revised per audit-derived amendment).**
   Build a model with two From-impl-for-LogArg Instances
   (`<LogArg as From<u32>>::from` and
   `<LogArg as From<&str>>::from`), but only the `From<u32>`
   wrapper is in `cx.diag_log_peel_residue.collapsed_wrapper_instances`.
   Call the pass. Assert (the widening clause per Change B
   step 1.2):
   - Both `<LogArg as From<T>>::from` Items are removed
     (residue widening: any From-impl whose Self is LogArg
     is dead when the residue accumulator is non-empty).
   - Both Instances are removed.
   - The LogArg ADT Item is removed (no surviving Self
     reference remains after the widening).
   - No surviving `Ty::Enum { item: <LogArg> }` entry
     remains in the TyInterner.

   This test locks the §26.6 contract on the realistic rustc-
   rlib materialisation pattern (the sibling From<&str> Instance
   exists because rustc emits every impl method in an rlib,
   not because the fixture calls `From<&str>`).

### Integration test (`crates/regseq-driver/tests/bridge_diag_log_dead_item_elim.rs`)

10. **`diag_logging` fixture -- LogArg fully removed
    (revised per audit-derived amendment).** Bridge the
    `crates/test-ir-fixtures/fixtures/diag_logging.rs`
    fixture. Assert the structural §26.6 contract via
    direct walks (NOT via `model.validate()` -- that
    assertion is deferred to the follow-up
    `regseq-driver-49` task that lands the validator's
    B-4/B-6 §26.6 exception per `spec.md:9650+` /
    `spec.md:11792+`; the step-0 intrinsic FnSig rewrite
    creates a per-Call arg / signature input-type
    mismatch that the current validator flags but the
    spec §23.6.2 exception explicitly permits):
    - Walk every `Item` in `model.items`; assert no
      `def_path.segments` terminal segment renders as
      `"LogArg"`. (D8 from regseq-model-94.)
    - Walk every `Ty` in the TyInterner via
      `model.types.iter()`; assert no
      `Ty::Enum/Struct/Union { item, .. }` where the
      resolved Item's def_path terminal segment renders
      as `"LogArg"`. (D7 from regseq-model-94.)
    - Walk every Function `Body.local_decls`; assert no
      LocalDecl `ty` resolves to a LogArg-named ADT.
    - Walk every `Instance`; assert no Instance's `item`
      resolves to a From-LogArg impl method.
    - The four post-peel log Calls remain present, with
      the canonical post-peel arity (1 fmt + 1 u32 per
      call).
    - The integration test file MUST include a header
      comment that explicitly cites `regseq-driver-49`
      as the follow-up that restores the
      `model.validate()` assertion. The comment serves as
      the documentation trail so a future audit
      cross-references the deferred-contract piece.

11. **`diag_logging` fixture -- snapshot.** Take an `insta`
    snapshot of `render_default(&model)` and lock in the
    post-elimination shape. The snapshot baseline must
    contain:
    - The `emit_logs` Function Instance.
    - Four `call ...` lines (one per severity, post-peel
      shape).
    - No `LogArg` identifier anywhere in the rendered output.
    - No `From::from` Function-instance rendering for the
      LogArg impls.

12. **Non-LogArg From impls in unrelated fixtures are
    preserved.** Bridge a fixture with a regular
    `pub struct S; impl From<u32> for S { ... }` AND a
    `pub fn use_s() -> S { S::from(1u32) }` entry point so
    the From impl is discovered + materialised. Assert the
    `From<u32> for S` impl Item + its Instance + its FnSig
    + `S`'s Ty are NOT removed. This locks in the narrow
    scope per Non-Goals.

### Updated existing tests

13. **`bridge_diag_log_peel.rs` + `bridge_diag_log_recognition.rs`**
    stay green. The integration tests use empty `&[]` slice
    shims that produce no LogArg::from wrappers; the
    elimination pass is a no-op for them.

14. **`regseq-model-94` (P1a-E2E-02) -- unblocked
    (audit-derived deferral).** Resetting and re-running
    `regseq-model-94` is **NOT** in scope for this task's
    completion: regseq-model-94's Test Plan begins with
    `model.validate().expect("validate must pass")` (its
    own Section A), which currently trips on the step-0
    intrinsic-FnSig-rewrite-induced arg/input mismatch.
    The reset + re-run sequence becomes applicable ONLY
    after the follow-up `regseq-driver-49` task lands the
    validator's §23.6.2 B-4 / B-6 §26.6 exception. Once
    that lands: `task.py reset regseq-model-94 --reason
    "regseq-driver-48 + regseq-driver-49 jointly unblock
    Test Plan §A validate() + §D LogArg-survival
    assertions"` and re-run unchanged.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-driver-48 against its task spec at
.harness/tasks/regseq-driver/48-dead-item-elimination-after-diag-log-peel.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-driver/src/bridge/diag_log/peel.rs (residue
  accumulator wiring + InstanceId extraction before wrapper-call
  collapse)
- crates/regseq-driver/src/bridge/diag_log/dead_item_elim.rs (new
  Phase-5 sub-pass with the fixed-point DCE computation)
- crates/regseq-driver/src/bridge/diag_log/mod.rs (any newly-
  shared helper lifted from peel.rs)
- crates/regseq-driver/src/bridge/mod.rs (BridgeCtxt accumulator
  field + pipeline wiring after const_memory_static synthesis)
- crates/regseq-model/src/model.rs (new
  remove_items_and_remap / remove_instances_and_remap /
  remove_tys_and_remap / remove_fn_sigs_and_remap methods +
  for_each_ty_id_mut + for_each_fn_sig_id_mut walkers)
- crates/regseq-driver/tests/bridge_diag_log_dead_item_elim.rs (new
  integration test per Test Plan items 10-12)
- Any new unit-test module added for Test Plan items 1-9

Focus on:
- Spec deviations vs spec.md sections 25.7.3 (DiagLogPeel
  contract), 26.4 (LogArg carrier-survival rule), 26.6 (the
  load-bearing 'LogArg type does NOT appear in RegseqModel'
  clause at spec.md:12705-12707).
- Residue accumulator: every collapsed wrapper Call's callee
  InstanceId is captured BEFORE collapse_wrapper_calls rewrites
  the terminator.
- DCE fixed-point computation: removal seeds are exactly the
  residue accumulator + the structural LogArg From-impl
  recognition; transitive removal proceeds only along chains
  anchored at those seeds (orphan unrelated unreferenced
  entries stay alive).
- TyId / FnSigId walker coverage: every Ty variant carrying
  TyId, every GenericArgs::Type, every body local_decl,
  statement, Rvalue, Const, Instance-kind TyId field; every
  FnSigId reference.
- Removal order in eliminate_diag_log_peel_residue:
  instances -> fn_sigs -> tys -> items (downstream removal
  depends on upstream's keep-set being computed first).
- Pipeline placement: elimination runs LAST in Phase 5 (after
  body passes + late-pending drain + sort_items_by_f4 + const
  static synthesis), before sort_instances_by_f4_2 in
  run_bridge.
- Post-removal: the TyInterner walk + the model.items walk
  find no LogArg reference. `model.validate()` is INTENTIONALLY
  SKIPPED in the integration test per the audit-derived
  amendment -- the step-0 intrinsic FnSig rewrite creates a
  per-Call arg-vs-signature input-type mismatch the spec
  §23.6.2 B-4 / B-6 §26.6 exception permits but the
  validator does not yet honour. Restoring the validate()
  assertion is owned by the follow-up `regseq-driver-49`
  task; verify the integration test file's header cites
  that follow-up.
- Step-0 intrinsic FnSig rewrite (Change B step 0) is gated
  on the residue accumulator being non-empty (idempotent
  no-op for empty-`&[]` fixtures).
- Test Plan item 9 asserts BOTH `<LogArg as From<T>>::from`
  Items are removed (sibling widening per Change B step 1.2),
  NOT that the sibling survives.
- HandleTable handling: bindings whose target is a removed
  Item are dropped from `cx.handles.handle_to_item_id`
  before remap; the dense remap maps removed slots to
  `ItemId::ZERO` (sentinel; unreachable after the drop).
- Test coverage covers each removal-candidate class explicitly,
  including the audit-revised sibling From impl test (item 9).
Do NOT flag anything explicitly marked out of scope (Non-Goals).
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
Total response <= 400 words.
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, form a verdict,
stop.
</default_follow_through_policy>
EOF
```

Iterate (fix -> re-invoke `codex_runner.py`) until VERDICT: APPROVED.

## Quality Gate

```
cargo test -p regseq-model
cargo test -p regseq-driver
cargo test -p test-ir-fixtures
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```

The authoritative gate is `task.py done`, which runs cleanup plus
workspace-wide test/fmt/clippy per `SESSION.md` Testing
Requirements. The above commands are dev-time guidance.

## Acceptance Criteria

- `crates/regseq-driver/src/bridge/diag_log/peel.rs` updated to
  push collapsed-wrapper InstanceIds onto the new
  `DiagLogPeelResidue` accumulator BEFORE
  `collapse_wrapper_calls` rewrites the terminators.
- `crates/regseq-driver/src/bridge/diag_log/dead_item_elim.rs`
  created with `eliminate_diag_log_peel_residue` per Change B,
  implementing the fixed-point DCE in dependency order
  (instances -> fn_sigs -> tys -> items).
- `crates/regseq-driver/src/bridge/diag_log/mod.rs` lifts the
  From-LogArg structural-recognition predicate from `peel.rs`
  to `pub(crate)` for shared use.
- `crates/regseq-driver/src/bridge/mod.rs::BridgeCtxt` gains a
  `diag_log_peel_residue: DiagLogPeelResidue` field; the new
  pass is wired into
  `construct_instances_and_resolve_cross_references` LAST in
  the Phase-5 sub-pass order (after `sort_items_by_f4` and
  `construct_const_memory_static_instances`).
- `crates/regseq-model/src/model.rs` adds:
  - `remove_items_and_remap` per Change C.
  - `remove_instances_and_remap` per Change C.
  - `remove_tys_and_remap` per Change C.
  - `remove_fn_sigs_and_remap` per Change C.
  - `for_each_ty_id_mut` walker per Change D.
  - `for_each_fn_sig_id_mut` walker per Change D.
- `crates/regseq-driver/tests/bridge_diag_log_dead_item_elim.rs`
  created with Test Plan items 10-12. The file header MUST
  cite the deferred `regseq-driver-49` follow-up that
  restores the `model.validate()` assertion (per the
  audit-derived amendment).
- Unit tests for Test Plan items 1-9 added.
- After this task lands: a new follow-up task
  `regseq-driver-49` is registered to land the validator's
  §23.6.2 B-4 / B-6 §26.6 exception (`spec.md:9650+` /
  `spec.md:11792+`) so the per-Call arg-vs-signature
  input-type mismatch the step-0 intrinsic FnSig rewrite
  introduces stops tripping `model.validate()`. ONLY after
  `regseq-driver-49` lands does this task's
  `task.py reset regseq-model-94 --reason ...`
  step apply -- regseq-model-94's Test Plan D assertions
  call `model.validate()` first, which currently fails.
- `cargo test --workspace` passes; in particular, bridging
  the `diag_logging` fixture yields a model with NO
  LogArg-typed Item, NO LogArg-typed Ty in the TyInterner,
  NO LogArg-typed LocalDecl, and NO surviving
  `<LogArg as From<T>>::from` Instance. The integration
  test does NOT assert `model.validate()` (deferred to
  `regseq-driver-49`).
- `cargo fmt --all -- --check` and
  `cargo clippy --workspace -- -D warnings` pass.
- Codex Task Review returns VERDICT: APPROVED.
