# regseq-driver-49: Validator B-4 / B-6 §26.6 Exception for Log-Intrinsic Calls + `instance_intrinsic` Implementation

## Description

Close the spec.md §23.6.2 B-4 / B-6 exception gap surfaced by
the regseq-driver-48 audit-fix sequence
(`crates/regseq-driver/src/bridge/diag_log/dead_item_elim.rs`'s
step-0 log-intrinsic FnSig rewrite). After
`eliminate_diag_log_peel_residue` runs, the four `log_<sev>`
intrinsic Function Instances carry rewritten FnSigs
`fn(&'static str,) -> ()` (the `&[LogArg<'_>]` input slot is
dropped per `regseq-driver-48` spec_version 2 Change B step 0),
but the surviving log Call terminators carry the post-
DiagLogPeel flat-arg shape `[fmt_literal, expr_1, ..., expr_N]`
(1 fmt + N peeled value-typed operands per spec.md §26.6).
`RegseqModel::validate`'s `validate_call`
(`crates/regseq-model/src/model.rs:2664-2811`) currently flags
this as a B-4 arity mismatch
(`crates/regseq-model/src/model.rs:2764-2771` --
`args.len() != inputs.len()` -> `CallArgsArityMismatch`) and
would also flag it as a B-6 per-arg type mismatch at line
2776-2787 if B-4 hadn't already short-circuited.

spec.md §23.6.2 (`spec.md:9568+`) explicitly pins the exception
for both invariants:

- **B-4 exception** (`spec.md:9649-9668`): when `callee =
  CallCallee::Direct(id)` and `model.instance_intrinsic(id) ==
  Some(IntrinsicKind::Log(_))`, the arity rule reframes from
  `args.len() == inputs.len()` to `args.len() == 1 + N` where
  `N` is the number of `{}` placeholders in the format
  template literal at `args[0]`.

- **B-6 exception** (`spec.md:9717-9768`): per-arg type match
  against the declared callee signature does NOT apply. The
  peeled shape's type invariants are:
  - `args[0]`: `Ty::Ref(Ty::Str, Provenance::SharedBorrow)`
    (the `&'static str` format-template operand). Literal-
    ness is validated at DiagLogPeel synthesis time
    (§25.7.3 step 1), not here.
  - `args[i]` for `i ∈ 1..args.len()`: value-typed `Ty` in
    the admissible LogArg-source set per §26.4 --
    `Ty::Uint(UintTy::U8..U64)`, `Ty::Int(IntTy::I8..I64)`,
    `Ty::Bool`, or `Ty::Ref(Ty::Str, _)`.

The B-4/B-6 exceptions require a working
`RegseqModel::instance_intrinsic` helper to identify Log-
intrinsic callees. The current implementation at
`crates/regseq-model/src/model.rs:1710-1712` is a `todo!()`
stub (with comment "P1a-DS-09 adds DomainAttrTables contents"
-- regseq-model-74, which was marked done, populated the data
but never wired up the accessor). The bridge populates
`domain_attrs.item_attrs[item]` with
`AttrData::Intrinsic(IntrinsicKind::Log(LogLevel))` entries
per regseq-driver-44's Phase-2 recognition codepath
(referenced at
`crates/regseq-driver/src/bridge/diag_log/peel.rs:578` and
populated at `regseq-driver-44` per spec.md §3.6.4). The
helper just needs to read that data.

After this task lands, `regseq-model-94`'s Test Plan Section A
(`model.validate(&bridge_input).expect("validate must pass")`)
becomes satisfiable on the post-elim `diag_logging` model, and
the deferred `task.py reset regseq-model-94` step from
`regseq-driver-48` spec_version 2 Test Plan item 14 applies.

## Input Context

- `docs/discussions/regseq-model/spec.md` §23.6.2 Positive
  invariants (`spec.md:9568-9778`), in particular:
  - B-4 arity rule + I-DiagLog-CallArity exception
    (`spec.md:9639-9668`)
  - B-6 per-arg type-match rule + I-DiagLog-CallArity
    exception (`spec.md:9695-9778`)
- `docs/discussions/regseq-model/spec.md` §26.4 (LogArg
  argument carrier + admissible source-type set;
  `spec.md:12481-12638`)
- `docs/discussions/regseq-model/spec.md` §26.6 (Bridge
  translation contract + `I-DiagLog-CallArity`;
  `spec.md:12692-12762`)
- `docs/discussions/regseq-model/spec.md` §3.5.2 (per-
  Instance intrinsic-kind pull-through helper; the spec
  citation for `instance_intrinsic`)
- `docs/discussions/regseq-model/spec.md` §3.6.1 + §3.6.4
  (`DomainAttrTables` + the Phase-2 recognition codepath that
  populates `item_attrs` with `IntrinsicKind::Log(_)`;
  `spec.md:1432+`)
- `crates/regseq-model/src/model.rs:1709-1712`
  (`instance_intrinsic` -- currently `todo!()` stub)
- `crates/regseq-model/src/model.rs:2664-2811`
  (`validate_call` -- the validator entry point this task
  extends with the B-4 / B-6 exceptions)
- `crates/regseq-model/src/model.rs:2764-2787` (the B-4
  arity check + B-6 per-arg type check; the load-bearing
  conditional this task adds the exception around)
- `crates/regseq-model/src/domain_attr.rs:11-46`
  (`DomainAttrTables` + `FieldPath`)
- `crates/regseq-model/src/domain_attr.rs:56-58`
  (`AttrData` enum + `AttrData::Intrinsic(IntrinsicKind)` variant)
- `crates/regseq-driver/src/bridge/diag_log/peel.rs:556-580`
  (`call_is_log_intrinsic` -- the bridge-side equivalent
  predicate; this task lifts the equivalent logic into a
  model-side helper)
- `crates/regseq-driver/tests/bridge_diag_log_dead_item_elim.rs:11-25`
  (the integration test whose header cites this task as the
  deferred follow-up that restores `model.validate()`)
- `.harness/tasks/regseq-driver/48-dead-item-elimination-after-diag-log-peel.md`
  spec_version 2 (the immediately-prior audit-fix that
  surfaced this gap; Test Plan item 14 conditions
  `regseq-model-94`'s reset on this task)
- `.harness/tasks/regseq-model/94-e2e-diagnostic-logging.md`
  (the downstream task this fix unblocks; Test Plan §A at
  line 217-224 is the `model.validate()` assertion this
  task makes satisfiable)
- `.harness/spec-batches/regseqmodel-phase-1a.md` (parent
  brief; this task is a phase-1a bridge follow-up addition
  beyond the original BR-01..BR-08b table)

## Implementation Notes

Three coupled changes. Change A is a prerequisite for Change
B (the validator needs the helper to identify Log callees).
Change C consumes Changes A + B end-to-end.

### Change A: Implement `RegseqModel::instance_intrinsic`

Replace the `todo!()` stub at
`crates/regseq-model/src/model.rs:1709-1712`:

```rust
/// Returns the closed-vocabulary intrinsic marker for an
/// instance's item per spec.md section 3.5.2. Walks
/// `domain_attrs.item_attrs[item]` for the first
/// `AttrData::Intrinsic(_)` entry. The bridge populates this
/// during Phase-2 translation per spec.md section 3.6.4 (see
/// `regseq-driver-44`'s Phase-2 recognition codepath).
///
/// Returns `None` when:
/// - `id` is out of range (defensive bound; surfaces as a
///   skip rather than a panic, mirroring the existing
///   bridge-side `call_is_log_intrinsic` discipline at
///   `crates/regseq-driver/src/bridge/diag_log/peel.rs:556-580`).
/// - The Instance's item has no `IntrinsicKind` entry in
///   `domain_attrs.item_attrs`.
/// - Bridge ordering: the Instance's `item.index() >=
///   domain_attrs.item_attrs.len()` (Phase-4 side-table
///   sizing; should not occur post-Phase-4 but guarded
///   defensively).
///
/// `item_attrs[item]` is a `Vec<AttrData>` that may carry
/// multiple per-domain payloads per item per
/// `crates/regseq-model/src/domain_attr.rs:27`; this helper
/// returns the first `AttrData::Intrinsic(_)` discriminant.
/// At most one `Intrinsic` entry per item is expected per
/// spec.md section 3.6.4 (the Phase-2 recognition codepath
/// pushes exactly one), but if multiple are present (a bridge
/// bug), the helper returns the FIRST -- downstream
/// validators read the discriminant only, so first-wins is
/// well-defined.
pub fn instance_intrinsic(&self, id: InstanceId) -> Option<&IntrinsicKind> {
    if id.index() >= self.instances.len() {
        return None;
    }
    let item = self.instances[id].item;
    if item.index() >= self.domain_attrs.item_attrs.len() {
        return None;
    }
    self.domain_attrs.item_attrs[item]
        .iter()
        .find_map(|attr| match attr {
            AttrData::Intrinsic(kind) => Some(kind),
        })
}
```

The `match` arm is exhaustive for the closed
`AttrData::Intrinsic(_)` enum per
`crates/regseq-model/src/domain_attr.rs:48+`. If `AttrData`
gains a non-Intrinsic variant in the future, the helper's
`find_map` skips it and continues looking for an Intrinsic
match -- which is the spec-correct behaviour (`instance_intrinsic`
returns the intrinsic-kind classification, not the full
attribute list).

Add `AttrData` and `IntrinsicKind` to the imports at the top
of `model.rs` if not already present.

Update the existing test
`crates/regseq-model/src/model_tests.rs:1131-1140`
(`instance_intrinsic_helper_signature_compiles`) to assert
real behaviour instead of just compile-time signature:

```rust
#[test]
fn instance_intrinsic_returns_log_kind_when_attr_present() {
    // Build a minimal model with one Instance whose Item
    // carries `AttrData::Intrinsic(IntrinsicKind::Log(_))`.
    // Assert `instance_intrinsic` returns Some(&Log(level)).
    // Assert returning None for an Instance whose Item has
    // empty item_attrs, and None for an out-of-range
    // InstanceId.
}
```

Add three sibling unit tests covering the three None cases
enumerated in the doc-comment.

### Change B: B-4 / B-6 §26.6 exception in `validate_call`

In `crates/regseq-model/src/model.rs::validate_call`
(`model.rs:2664-2811`), insert a precondition check
immediately after `resolved_sig` is bound
(`model.rs:2759-2761`) but BEFORE the B-4 arity check at line
2764: detect Log-intrinsic Direct callees and route them
through a dedicated `validate_log_intrinsic_call` helper
instead of the generic B-4 / B-6 walk.

```rust
let Some((inputs, output)) = resolved_sig else {
    return Ok(());
};

// I-DiagLog-CallArity exception per spec.md section 23.6.2
// B-4 / B-6 (`spec.md:9649-9778`): Log-intrinsic Direct
// callees carry the post-DiagLogPeel flat-arg shape
// `[fmt_literal, expr_1, ..., expr_N]`, which does NOT match
// the declared callee FnSig's `(fmt, &[LogArg<'_>])` inputs
// nor the post-regseq-driver-48 rewritten `(fmt,) -> ()`
// inputs. Route Log-intrinsic callees through the dedicated
// validator that honours the peeled shape's invariants for
// B-4 + B-6 only; B-3 (destination type) + B-9 (target /
// divergence coherence) at `model.rs:2802-2848` still apply
// uniformly because the post-rewrite callee FnSig output is
// `unit` (matching the post-peel destination place) and
// `target = Some(_)` (the Call is not divergent).
let is_log_intrinsic = match callee {
    CallCallee::Direct(id) => {
        matches!(self.instance_intrinsic(*id), Some(IntrinsicKind::Log(_)))
    }
    CallCallee::Indirect(_) => false,
};
if is_log_intrinsic {
    self.validate_log_intrinsic_call(owner, block, body, args)?;
} else {
    // B-4: arity match (existing).
    if args.len() != inputs.len() { /* existing */ }
    // B-6: per-arg type match (existing).
    for (i, arg) in args.iter().enumerate() { /* existing */ }
}

// B-9 + B-3 checks at `model.rs:2802-2848` fall through
// uniformly for both branches.
```

Add a new method `validate_log_intrinsic_call` next to
`validate_call`:

```rust
/// Validate a Log-intrinsic Call per spec.md section 23.6.2
/// B-4 / B-6 I-DiagLog-CallArity exception (`spec.md:9649-9778`).
/// Honours the post-DiagLogPeel flat-arg shape per spec.md
/// section 26.6:
/// - `args[0]` is `Ty::Ref(Ty::Str, Provenance::SharedBorrow)`
///   (the `&'static str` format template).
/// - `args[i]` for `i in 1..args.len()` is a value-typed Ty
///   in the admissible LogArg-source set per spec.md section
///   26.4: `Ty::Uint(UintKind::U8..U64)`,
///   `Ty::Int(IntKind::I8..I64)`, `Ty::Bool`, or
///   `Ty::Ref(Ty::Str, _)`.
/// - Arity: `args.len() >= 1` (at least the fmt slot).
///   Stronger arity-vs-`{}`-placeholder-count check (`args.len()
///   == 1 + N`) is performed at DiagLogPeel synthesis time
///   per `crates/regseq-driver/src/bridge/diag_log/peel.rs::validate_log_call_fingerprints`
///   (D1 per spec.md section 26.8); the validator does NOT
///   re-derive the placeholder count from the literal because
///   the model does not pin the literal's content (the bytes
///   live in a ConstMemoryStatic per section 9.7 and are not
///   accessible without bridge-side const-memory-cache state).
///   This validator runs structural shape checks only.
fn validate_log_intrinsic_call(
    &self,
    owner: InstanceId,
    block: BlockId,
    body: &Body,
    args: &[Operand],
) -> Result<(), InvariantViolation> { /* ... */ }
```

The helper's implementation:

1. **Empty-args defensive check (FIRST)**: `args.is_empty()`
   returns `InvariantViolation::DiagLogCallArityBelowMinimum
   { owner, block }` (new variant). This MUST run before any
   `args[0]` access; the post-DiagLogPeel contract guarantees
   `args.len() >= 1` per spec.md §26.6, but the validator's
   discipline is "validate, do not assume" and an empty-args
   `args[0]` access would panic.

2. **`args[0]` shape check**: `operand_ty(self, body, &args[0])`
   must resolve (per the `TypeRecovery` enum) to
   `Ty::Ref(_, Provenance::SharedBorrow)` whose pointee
   resolves to `Ty::Str`. On a Recovered mismatch, return
   `InvariantViolation::DiagLogCallArg0NotStrRef { owner, block,
   actual_ty }` (new variant; see "Error variants" below).
   On Malformed, return
   `InvariantViolation::DiagLogCallArg0NotStrRef { ..,
   actual_ty: TyId::ZERO }` (placeholder-typed; matches the
   existing `TypeRecovery::Malformed` discipline at
   `model.rs:2743-2749`). On NotApplicable (constant whose Ty
   is not recoverable from inline data per §20.5), skip the
   check.

3. **Per-position admissibility for `args[1..]`**: for each
   `i in 1..args.len()`, the operand's resolved Ty must be in
   the admissible-LogArg-source set:
   - `Ty::Uint(UintKind::U8 | U16 | U32 | U64)`
   - `Ty::Int(IntKind::I8 | I16 | I32 | I64)`
   - `Ty::Bool`
   - `Ty::Ref(pointee, _)` where `pointee` resolves to `Ty::Str`

   On Recovered mismatch, return
   `InvariantViolation::DiagLogCallArgNotAdmissible { owner,
   block, idx, actual_ty }`. On Malformed, return the same
   with `TyId::ZERO`. On NotApplicable, skip (operand-context
   typing is upstream).

4. Return `Ok(())` when all checks pass.

#### Error variants

Add three new variants to `InvariantViolation`
(`crates/regseq-model/src/model.rs::InvariantViolation` --
the enum at the file's top):

```rust
/// Log-intrinsic Call's args[0] is not `Ty::Ref(Ty::Str, _)`
/// per spec.md section 26.6 (`spec.md:12705+`).
DiagLogCallArg0NotStrRef {
    owner: InstanceId,
    block: BlockId,
    actual_ty: TyId,
},

/// Log-intrinsic Call's args is empty (post-DiagLogPeel must
/// guarantee at least the format-template slot).
DiagLogCallArityBelowMinimum {
    owner: InstanceId,
    block: BlockId,
},

/// Log-intrinsic Call's args[idx] (idx >= 1) does not
/// resolve to a value-typed Ty in the spec.md section 26.4
/// admissible-LogArg-source set.
DiagLogCallArgNotAdmissible {
    owner: InstanceId,
    block: BlockId,
    idx: usize,
    actual_ty: TyId,
},
```

Add matching `InvariantId::*` discriminants per the existing
pattern at `model.rs::InvariantId`. The descriptor strings
follow the spec-pinned wording at `model.rs::invariant_descriptor`.

### Change C: Restore `model.validate()` in integration test + reset `regseq-model-94`

In `crates/regseq-driver/tests/bridge_diag_log_dead_item_elim.rs`:

1. Replace the header comment block that defers
   `model.validate()` to this task (lines 11-26 of the
   current file) with a single-line comment noting this
   task has landed and validate() is now asserted.

2. Restore the `model.validate()` assertion at the top of
   `diag_logging_post_elim_removes_every_logarg_artifact`.
   `RegseqModel::validate` is `validate(&mut self,
   bridge_input: &BridgeInput)` per
   `crates/regseq-model/src/model.rs:1455`; the test must
   bind `let mut model = ...`, construct
   `let bridge_input = regseq_model::BridgeInput::matching(&model);`,
   then call:

   ```rust
   model.validate(&bridge_input)
       .expect("validate succeeds post-elim per regseq-driver-49's B-4 / B-6 §26.6 exception");
   ```

   This is the FIRST assertion in the test (after the
   bridge run completes). The existing structural walks (no
   LogArg Item, no LogArg Ty, etc.) stay as-is.

3. The remaining structural walks (no LogArg in items, no
   LogArg in TyInterner, etc.) stay as-is -- they continue
   to lock the §26.6 contract end-to-end.

Reset `regseq-model-94` after this task lands per the
`regseq-driver-48` spec_version 2 Test Plan item 14
deferred sequence:

```
task.py reset regseq-model-94 --reason \
  "regseq-driver-48 + regseq-driver-49 jointly unblock \
   Test Plan §A validate() + §D LogArg-survival assertions"
```

### Conventions

- Doc comments on every new public item (per `SESSION.md`
  Coding Conventions).
- No unicode symbols in source / docs.
- The new `InvariantViolation` variants reuse the existing
  `{ owner: InstanceId, block: BlockId, ... }` shape so the
  diagnostic surface composes with the existing per-Call
  error fields at `model.rs::InvariantViolation`.
- All assertions cite the `spec.md` section they verify in
  a one-line comment so the Codex review can cross-reference.
- The validator is read-only over `self` -- this task does
  NOT modify the model state during validation. The B-4 / B-6
  exception is a check-then-skip, not a rewrite.

## Non-Goals

- **D1 placeholder-count vs arg-count check.** spec.md
  §26.6's "args.len() == 1 + N where N is the `{}`-
  placeholder count in the format template" rule is the
  STRONGER form of B-4. The validator runs structural
  shape checks only and does NOT re-derive the placeholder
  count from the literal (the bytes live in a
  ConstMemoryStatic per §9.7 and are not accessible to a
  read-only `&self` walk). The placeholder-count check is
  owned by `regseq-driver-45`'s
  `crates/regseq-driver/src/bridge/diag_log/peel.rs::validate_log_call_fingerprints`
  (D1 per spec.md §26.8), which has access to the
  `const_memory_static_cache` bridge state.
- **Modification of `RegseqModel::validate_ty_interner`.**
  The TyInterner walk at `model.rs:1947+` does not need
  a Log-intrinsic exception -- it walks every interned Ty
  for structural validity (`Ty::Enum/Struct/Union` items
  resolve to in-range ItemIds, layouts are well-formed,
  etc.), none of which depend on Log-intrinsic semantics.
- **Modification of `regseq-driver-48`'s implementation.**
  That task's load-bearing changes (step-0 FnSig rewrite,
  widened seed, HandleTable drop+remap) stay verbatim;
  this task adds the matching validator exception so
  `model.validate()` succeeds on the post-elim model.
- **Re-architecting the validator pipeline.** The exception
  routes Log-intrinsic Direct callees through a dedicated
  helper before the generic B-4 / B-6 path. No other
  validator surface changes.
- **Bridge-side recognition refinement.** The bridge's
  Log-intrinsic-recognition codepath
  (`crates/regseq-driver/src/bridge/diag_log/peel.rs::call_is_log_intrinsic`)
  is unchanged. The model-side `instance_intrinsic` helper
  is the single source of truth for the validator; the
  bridge-side predicate composes through the same
  `domain_attrs.item_attrs` data per spec.md §3.6.1.
- **Indirect callees with Log-intrinsic semantics.**
  spec.md §23.6.2 B-5 explicitly forbids this:
  "`IntrinsicKind` is keyed by Instance identity, and
  indirect callees do not pin an InstanceId." The
  exception applies only to `CallCallee::Direct(_)` -- the
  precondition in Change B's pseudocode reflects this.

## Test Plan

### Unit tests (`crates/regseq-model/src/model_tests.rs`)

1. **`instance_intrinsic` returns Some when AttrData is
   present.** Build a minimal model with one Instance whose
   Item has `domain_attrs.item_attrs[item] =
   vec![AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Info))]`.
   Assert `instance_intrinsic(id) ==
   Some(&IntrinsicKind::Log(LogLevel::Info))`.

2. **`instance_intrinsic` returns None when no AttrData.**
   Same model shape but with an empty `item_attrs[item]`
   Vec. Assert `instance_intrinsic(id) == None`.

3. **`instance_intrinsic` returns None for out-of-range
   InstanceId.** Assert
   `instance_intrinsic(InstanceId::from_u32(999)) == None`
   on a model with fewer than 999 Instances.

4. **`instance_intrinsic` is stable across InstanceId
   permutations.** Build a model, permute Instances via
   `sort_instances_by_f4_2`, assert
   `instance_intrinsic(new_id) == instance_intrinsic(old_id)`
   for the remapped IDs.

5. **`validate_log_intrinsic_call` accepts the canonical
   post-peel shape.** Build a model with a Log-intrinsic
   Call whose `args` = `[Operand::Constant(<&str>),
   Operand::Move(<u32_local>)]`. Assert
   `validate_call(...)` returns `Ok(())`. The fmt const
   resolves to `Ty::Ref(Ty::Str, SharedBorrow)`; the u32
   operand resolves to `Ty::Uint(UintKind::U32)` per
   admissible set.

6. **Args[0] non-`&str` rejection.** Build a Log-intrinsic
   Call whose `args[0]` resolves to `Ty::Uint(U32)` (not a
   `&str` literal). Assert `validate_call(...)` returns
   `Err(InvariantViolation::DiagLogCallArg0NotStrRef { ..,
   actual_ty: <U32 ty_id> })`.

7. **Args[1..] non-admissible rejection.** Build a Log-
   intrinsic Call whose `args[1]` resolves to a
   non-admissible Ty (e.g., `Ty::Tuple { fields: [], .. }`
   for unit, or `Ty::Float(_)`). Assert
   `validate_call(...)` returns
   `Err(InvariantViolation::DiagLogCallArgNotAdmissible {
   idx: 1, .. })`.

8. **Empty-args defensive rejection.** Build a Log-
   intrinsic Call whose `args` is empty (post-peel must
   guarantee `args.len() >= 1`; this case asserts the
   defensive check fires). Assert
   `validate_call(...)` returns
   `Err(InvariantViolation::DiagLogCallArityBelowMinimum { .. })`.

9. **Non-Log Direct Call uses generic B-4 / B-6 path.**
   Build a Direct Call to a non-Log Function Instance with
   one input and two args (deliberate arity mismatch).
   Assert `validate_call(...)` returns
   `Err(InvariantViolation::CallArgsArityMismatch { .. })`
   (the existing B-4 path fires; the exception did NOT
   short-circuit). This locks in the "exception applies
   only to Log-intrinsic callees" contract.

10. **Indirect Call ignores the exception.** Build an
    Indirect Call whose `operand_ty` resolves to `Ty::FnPtr(_)`
    pointing at a FnSig that happens to match the Log-
    intrinsic shape. Assert the generic B-4 / B-6 path
    fires (the exception does not apply to Indirect
    callees per spec.md §23.6.2 B-5).

11. **`Ty::Ref(Ty::Str, _)` accepted at args[i>=1]**.
    Build a Log-intrinsic Call whose `args[1]` resolves to
    `Ty::Ref(Ty::Str, Provenance::SharedBorrow)` (a string-
    valued operand per the §26.4 admissible set). Assert
    `validate_call(...)` returns `Ok(())`.

12. **Boolean and integer-width admissibility.** Parametric
    test that builds Log-intrinsic Calls for each admissible
    args[1] Ty (`Ty::Bool`, `Ty::Uint(U8)`, `Ty::Uint(U16)`,
    `Ty::Uint(U32)`, `Ty::Uint(U64)`, `Ty::Int(I8)`,
    `Ty::Int(I16)`, `Ty::Int(I32)`, `Ty::Int(I64)`) and
    asserts all are accepted.

### Integration test update (`crates/regseq-driver/tests/bridge_diag_log_dead_item_elim.rs`)

13. **`diag_logging_post_elim_removes_every_logarg_artifact`
    asserts `model.validate()` succeeds.** Per Change C,
    insert `model.validate(&bridge_input).expect("validate
    succeeds post-elim per regseq-driver-49's B-4 / B-6
    §26.6 exception")` as the first assertion in the test
    (after the bridge run completes). The existing
    structural walks (no LogArg Item, no LogArg Ty, no
    LogArg LocalDecl, no surviving From-LogArg Instance,
    four post-peel log Calls with canonical arity) stay
    as-is.

14. **`diag_logging_post_elim_render_snapshot` unchanged.**
    The snapshot test is structural; this task's validator
    update does not affect the rendered model.

15. **`non_logarg_from_impl_in_unrelated_fixture_is_preserved`
    unchanged.** This task does not touch the residue-driven
    DCE; the preservation contract still holds.

### Downstream regseq-model-94 reset (executed AFTER this task lands)

16. **`regseq-model-94` Test Plan A1-F15 passes end-to-end.**
    After this task's commit lands, run
    `task.py reset regseq-model-94 --reason ...` and re-run
    the task unchanged. All assertions (A: validate; B: four
    Log Calls located; C: post-peel arity + flat-arg shape;
    D: LogArg-survival negatives; E: IntrinsicKind::Log
    attribute population; F: snapshot) pass under the
    composed regseq-driver-45 / 47 / 48 / 49 chain.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-driver-49 against its task spec at
.harness/tasks/regseq-driver/49-validator-diag-log-call-arity-exception.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/model.rs (new instance_intrinsic
  implementation + new validate_log_intrinsic_call helper +
  validate_call exception precondition + new InvariantViolation
  variants + matching InvariantId discriminants + descriptor
  strings)
- crates/regseq-model/src/model_tests.rs (unit tests per Test
  Plan items 1-12)
- crates/regseq-driver/tests/bridge_diag_log_dead_item_elim.rs
  (header comment update + restored model.validate() assertion
  per Test Plan item 13)

Focus on:
- Spec deviations vs spec.md sections 23.6.2 (B-4 / B-6 +
  I-DiagLog-CallArity exception at spec.md:9649-9778),
  26.4 (admissible LogArg-source set), 26.6 (the post-
  DiagLogPeel flat-arg shape contract at spec.md:12705+).
- `instance_intrinsic` correctness: reads
  `domain_attrs.item_attrs[item]` for the first
  `AttrData::Intrinsic(_)` entry; defensive None for
  out-of-range InstanceId AND out-of-range item.index() vs
  item_attrs.len().
- Exception precondition placement: runs AFTER
  `resolved_sig` is bound but BEFORE the generic B-4 / B-6
  walk; applies only to `CallCallee::Direct(_)` (NOT
  Indirect per spec.md section 23.6.2 B-5).
- `validate_log_intrinsic_call` checks: args[0] is
  `Ty::Ref(Ty::Str, _)` (or the existing TypeRecovery::
  Malformed / NotApplicable discipline); args[1..] is in
  the admissible-LogArg-source set per spec.md section
  26.4; args.len() >= 1 defensive check.
- New InvariantViolation variants follow the existing
  per-Call diagnostic shape (`owner: InstanceId, block:
  BlockId, ...`).
- Test coverage covers each rejection path AND each
  acceptance path explicitly; the generic-B-4-still-fires
  test (item 9) and the Indirect-callee-ignored test
  (item 10) lock the exception's narrow scope.
- Integration test restores `model.validate()` per Change C.
- Non-Goals are respected: NO D1 placeholder-count check,
  NO TyInterner exception, NO modification of DiagLogPeel
  or DCE, NO reuse of bridge-side `call_is_log_intrinsic`.
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

The authoritative gate is `task.py done`, which runs cleanup
plus workspace-wide test/fmt/clippy per `SESSION.md` Testing
Requirements. The above commands are dev-time guidance.

## Acceptance Criteria

- `crates/regseq-model/src/model.rs::instance_intrinsic`
  implemented per Change A (replaces the `todo!()` stub at
  `model.rs:1709-1712`).
- `crates/regseq-model/src/model.rs::validate_call` extended
  per Change B with the I-DiagLog-CallArity exception
  precondition before the generic B-4 / B-6 walk.
- `crates/regseq-model/src/model.rs::validate_log_intrinsic_call`
  helper added per Change B implementing the args[0] +
  args[1..] + arity-floor checks.
- Three new `InvariantViolation` variants
  (`DiagLogCallArg0NotStrRef`, `DiagLogCallArityBelowMinimum`,
  `DiagLogCallArgNotAdmissible`) + matching `InvariantId`
  discriminants + `invariant_descriptor` strings added.
- `crates/regseq-model/src/model_tests.rs` unit tests per
  Test Plan items 1-12 added; the existing stub at
  `model_tests.rs:1131-1140`
  (`instance_intrinsic_helper_signature_compiles`) replaced
  by Test Plan item 1's real-behaviour test.
- `crates/regseq-driver/tests/bridge_diag_log_dead_item_elim.rs`
  header comment updated + `model.validate()` assertion
  restored per Change C.
- `cargo test --workspace` passes; in particular,
  `bridge_diag_log_dead_item_elim.rs`'s integration test
  now asserts `model.validate()` returns `Ok(())` on the
  post-elim `diag_logging` model.
- `cargo fmt --all -- --check` and
  `cargo clippy --workspace -- -D warnings` pass.
- Codex Task Review returns VERDICT: APPROVED.
- After this task lands: `task.py reset regseq-model-94
  --reason "regseq-driver-48 + regseq-driver-49 jointly
  unblock Test Plan §A validate() + §D LogArg-survival
  assertions"` and re-run that task unchanged. All Test
  Plan A1-F15 assertions pass.
