# Inbound expectations: regseq-driver-55

## From regseq-driver-55 (2026-05-19T00:00:00Z)

Self-filed during the spec-v2 implementation of Change G. The v2
amendment (separate `validate_bridge_completeness` entry point
invoked from `bridge::run_bridge`) was implemented correctly and
all of its own Test Plan items pass (A1-A9, B1-B3, C1-C2, D1-D3
in isolation; the v1 partial-model-corpus regression is resolved
by the un-fold). However, invoking the spec'd Change-G boolean
from `run_bridge` surfaces a pre-existing source-`Function`
coverage gap that this task is NOT scoped to fix and that the
spec'd boolean cannot tolerate. Spec v3 must resolve the
following before regseq-driver-55 can land.

### The conflict

Change G (spec v2) + Acceptance Criteria mandate: invoked from
`run_bridge`, for every Item whose `kind` is one of
`{ Function, Static, Const, Struct, Enum, Union }` AND
`item_synthetic_kinds[i] == None` AND `linkage != Foreign`, there
must exist >= 1 covering Instance. The Acceptance Criteria
sanction ONLY two carve-outs: synthetic Items and foreign Items.
`Function` is explicitly in the kind set; no Function/RPIT/
uncalled carve-out is permitted.

Repro (the pre-existing `crates/regseq-driver/tests/bridge_synth.rs`
fixture `run_bridge_synthesises_closure_vtable_and_drop_glue_fixtures`):

```rust
pub fn closure_fixture() -> impl Fn(u32) -> u32 { |x| x + 1 }
```

`closure_fixture` is exported (rlib) so the Phase-2 pre-pass
interns it as a source `ItemKind::Function` Item
(`SyntheticItemKind::None`, `Linkage::Default`).
`construct_function_instances`
(`crates/regseq-driver/src/bridge/mod.rs`, the
"symmetric source-Function precedent" this task explicitly does
NOT modify) skips it: `translate_fn_sig` on the declared
signature `fn() -> impl Fn(u32) -> u32` returns
`Err(BridgeError::NonRigidTy { kind: Alias, ty_repr: "impl Fn(u32)
-> u32" })` because the RPIT return is a non-rigid
`Alias(Opaque)`. That `let Ok(fn_sig) = ... else { continue; }`
skip is documented, intentional bridge behavior, with lazy
call-site Instance creation via `lookup_or_create_direct_instance`.
`closure_fixture` is never called as a direct callee, so no lazy
Instance is created either. Net: a source-derived non-foreign
`ItemKind::Function` Item with ZERO Instances -- a state the
bridge's own documented `construct_function_instances` design
treats as legitimate, but spec v2 Change G flags as a hard
`InvariantViolation::ItemMissingInstance` from `run_bridge`,
regressing a pre-existing test (Acceptance Criterion
"`cargo test --workspace` regression-clean" fails).

### Why this is out of scope for regseq-driver-55

The Description enumerates exactly SIX gaps to close
(source struct/enum/union, synth VTableType-struct, source
static, source const) and treats source Functions as the
ALREADY-handled `construct_function_instances` precedent. No
enumerated gap and no Change (A-G) touches Function-Instance
construction. Making uncalled / RPIT-return source `fn` Items
eagerly covered would require modifying the precedent pass and/or
the `NonRigidTy` opaque-return rejection -- out of scope and
explicitly excluded by the task framing. Excluding `Function`
from the Change-G kind set is a unilateral deviation from an
explicit, Codex-reviewed spec clause (the v2 AMENDMENT APPROVED
verdict) and would be flagged by the mandatory Codex Task Review
(whose verbatim prompt asserts the `{ Function, Static, Const,
Struct, Enum, Union }` kind set and sanctions only the foreign +
synthetic carve-outs).

### Adjudication options for spec v3 (non-prescriptive)

1. Scope Change G's coverage to the kinds this task eagerly
   constructs (`Static`/`Const`/`Struct`/`Enum`/`Union`),
   delegating Function-Instance coverage to the existing
   invariant-10 `FunctionBodyPresenceMatchesKind` family + the
   documented lazy-Instance design. (B1-B3 already use `Struct`,
   so Test Plan B is unaffected.)
2. Add an explicit Change-G carve-out for source `Function`
   Items whose declared signature is non-rigid-opaque (RPIT) or
   that have no eager/lazy Instance per the documented
   `construct_function_instances` lifecycle.
3. Open a separate `regseq-driver-NN` to make source-`Function`
   eager-Instance construction total (reveal/normalize RPIT
   `Alias(Opaque)` return types in `construct_function_instances`
   so exported uncalled RPIT fns get an Instance), and gate
   regseq-driver-55's `run_bridge` Change-G invocation on it.

### State preserved on the branch

The correct v2 implementation is committed for adjudication
(mirrors the v1 -> v2 precedent): `validate_bridge_completeness`
as a separate public entry point on `RegseqModel` (NOT folded
into `validate`); the v1 partial-model-corpus regression
resolved; Test Plan B rewired to the new entry point with the
B1 permissive-`validate` cross-check; Test Plan C2 asserting
both entry points; the D1 `e2e_synthesis__vtable_render.snap`
re-baselined to the intended two-new-struct-definition drift.
The ONLY failing workspace test is the `bridge_synth`
`closure_fixture` case above.
