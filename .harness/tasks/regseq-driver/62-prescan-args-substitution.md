# regseq-driver-62: Synthesis pre-scan substitutes `Instance.args` into MIR

## Description

Fix the same root-cause class regseq-driver-60 closed in the body
materializer, but in a different pass: the Phase-2 synthesis
pre-scan at
`crates/regseq-driver/src/bridge/mod.rs:762-781` walks
`tcx.optimized_mir(local_def_id)` -- the POLYMORPHIC body -- for
every monomorphized `MonoItem::Fn` Instance, then runs
`scan_terminator_for_synthesis` on each Call terminator, which
calls `canonicalize_call_site_callee` ->
`Instance::try_resolve` (at
`crates/regseq-driver/src/bridge/mod.rs:2447`). For a Call site
that resolves to a trait method with a generic `Self`, `try_resolve`
returns `Ok(None)` because rustc cannot resolve a trait method on a
generic `Param`. The bridge surfaces this as
`BridgeError::Phase4(...)` at `bridge/mod.rs:1042-1046`.

Manual stress-testing surfaced this on the canonical
closure-indirection shape (reproduced inline below; the
implementer authors the same shape as the B2 fixture):

```rust
#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

pub fn apply(x: i32, y: i32, f: impl Fn(i32, i32) -> i32) -> i32 {
    f(x, y)
}

pub fn entry(x: i32, y: i32) -> i32 {
    apply(x, y, |x, y| x + y)
}
```

The failure trace:

```
error: regseq-driver: bridge failed: phase 4 assignment failed:
  call-site Instance::try_resolve failed for
  DefId(... Fn::call) with args
  [impl Fn(i32, i32) -> i32/#0, (i32, i32)]:
  Instance::try_resolve returned Ok(None)
```

Root cause:

1. `discovery::collect_crate_mono_items` produces `MonoItem::Fn(
   apply::<{closure}>)` -- a fully-monomorphized Instance with
   `instance.args == [{closure_type}]`.
2. The pre-scan loop at `bridge/mod.rs:762-781` fetches
   `tcx.optimized_mir(apply_def_id)` at line 769 -- the
   POLYMORPHIC body of `apply`. `apply`'s body contains a Call
   terminator for `<F as Fn>::call(&f, (x, y))`, where `F` is
   still `TyKind::Param(0)`.
3. `scan_terminator_for_synthesis` at line 778 reaches
   `canonicalize_call_site_callee` at `bridge/mod.rs:1039`,
   passing `(Fn::call_def_id, [Param(0), (i32, i32)])`.
4. `Instance::try_resolve` cannot resolve the trait method on a
   generic `Self` and returns `Ok(None)`.
5. The bridge converts this to `BridgeError::Phase4(...)` at
   `bridge/mod.rs:1042-1046` and fails.

This is the IDENTICAL root-cause shape as regseq-driver-60
(closed at `a1ac768`): process polymorphic MIR for a monomorphized
Instance without applying `instance.args`. driver-60 fixed
`materialise.rs:243`; this task fixes the parallel bug in the
synthesis pre-scan. The pattern is the same; the fix mirrors
driver-60's Change A applied to a different code path.

This is a focused production-code fix to the synthesis pre-scan.
No spec amendment, no schema change, no validator change.

## Input Context

### Spec anchors

- `docs/discussions/regseq-model/spec.md` §8.1 call-site
  canonicalization (`spec.md:2360-2365` -- the regseq-driver-56
  normative contract for `Instance::try_resolve` at Call sites).
  The contract holds for monomorphized call sites; the bug is
  that the pre-scan feeds a polymorphic call site into the
  resolver. (`spec.md:2351-2358` is the regseq-driver-58 Change H
  amendment for default trait method bodies; unrelated to this
  task's surface.)
- `docs/discussions/regseq-model/spec.md` §9 Instance and
  InstanceKind (`spec.md:2436`). The substitution semantics
  carried in `Instance.args` are the normative shape this task
  exploits to monomorphize the body.
- `docs/discussions/regseq-model/spec.md` §15 Bridge translation
  rules (`spec.md:5890`). The bridge's invariant is that it
  operates on fully-monomorphized MIR; this task closes the gap
  where the synthesis pre-scan was violating that invariant.

### Bridge code anchors (this task's edit surface)

- `crates/regseq-driver/src/bridge/mod.rs:762-781` -- the
  synthesis pre-scan loop. Line 769 is the
  `tcx.optimized_mir(local_def_id)` call that fetches the
  polymorphic body; replace with a substitution-applying variant.
- `crates/regseq-driver/src/bridge/mod.rs:1015-1077` --
  `scan_terminator_for_synthesis`, called at line 778. Behavior
  is unchanged after the fix; the input body now carries
  monomorphized terminators so `canonicalize_call_site_callee`
  sees concrete `generic_args` instead of `Param(_)`.
- `crates/regseq-driver/src/bridge/mod.rs:1039-1047` -- the
  `Instance::try_resolve` call-site canonicalization invocation
  that today surfaces the `Ok(None)` -> `BridgeError::Phase4`
  conversion. UNCHANGED after this fix; the resolver simply
  starts seeing resolvable inputs.
- `crates/regseq-driver/src/bridge/mod.rs:2441-2482` --
  `canonicalize_call_site_callee`. UNCHANGED.
- `crates/regseq-driver/src/bridge/body/materialise.rs:287-293` --
  the regseq-driver-60 fix site. This task applies the same
  pattern (substituted-body fetch) to the pre-scan path. Reference
  the driver-60 implementation for the API + clone semantics.

### Prior tasks

- regseq-driver-60 (closed at `a1ac768`): body materializer
  args-substitution. The same root-cause shape, fixed in a
  different pass. The pre-scan bug was not addressed because
  driver-60's spec explicitly Non-Goal'd "Phase-2/Phase-5 passes
  other than the body materializer" -- a scoping decision that
  this task corrects for the synthesis pre-scan specifically.
- regseq-driver-61 (closed at `98bc006`): `sig_c_abi_lowerings`
  parallel-table sync. Unrelated to this task but the most
  recent close; sets the dependency chronology.

### Test surface

- `crates/test-ir-fixtures/src/lib.rs:188-198` -- the
  `bridge_fixture` helper already injects `-C panic=abort`. New
  fixtures authored under this task automatically get the right
  panic-strategy treatment.
- The committed regression guard goes in test-ir-fixtures per the
  B2 plan below. The bug's source-shape repro is captured inline
  in the Description block above and re-used in B2; the
  implementer does NOT depend on any uncommitted file under
  `tests/fixtures/playground/` for repro authoring.

## Implementation Notes

### Change A: substitute `instance.args` into the body before scanning

**Anchor**: `crates/regseq-driver/src/bridge/mod.rs:769`.

Replace the polymorphic body fetch with the substitution-applying
variant, mirroring regseq-driver-60's body materializer fix.

**Current code** (~lines 762-781):

```rust
for mono_item in discovered_mono_items {
    let MonoItem::Fn(instance) = mono_item else {
        continue;
    };
    let Some(local_def_id) = instance.def_id().as_local() else {
        continue;
    };
    let body = ctxt.input.tcx.optimized_mir(local_def_id);
    for local in &body.local_decls {
        scan_ty_for_synthesis(ctxt, local.ty)?;
    }
    for block in body.basic_blocks.iter() {
        for statement in &block.statements {
            scan_statement_for_synthesis(ctxt, &body.local_decls, &statement.kind)?;
        }
        if let Some(terminator) = &block.terminator {
            scan_terminator_for_synthesis(ctxt, &body.local_decls, &terminator.kind)?;
        }
    }
}
```

Structural problem: line 769 fetches the polymorphic body; the
`Instance.args` field carrying the substitution is dropped on the
floor; the inner loops at lines 770-779 then feed polymorphic
local_decls/statements/terminators into the scan functions,
which trips `canonicalize_call_site_callee` on any trait-method
call site that uses a `Param` `Self`.

**Target shape**:

1. Fetch the polymorphic body via `tcx.instance_mir(instance.def)`
   (NOT `tcx.optimized_mir(local_def_id)` -- per regseq-driver-60
   Change A step 2, `instance_mir` returns the right MIR for the
   instance kind: source body for Item, shim body for
   DropGlue / VTableShim / etc.).
2. Substitute `instance.args` into the body using
   `tcx.instantiate_and_normalize_erasing_regions`. The cloned-
   body cost is acceptable -- one body clone per monomorphized
   Instance, comparable to the per-Instance overhead in
   driver-60's fix.
3. Feed the substituted body's local_decls and basic_blocks into
   the existing scan loops UNCHANGED.

**Recommended implementation**:

```rust
use rustc_middle::ty::{EarlyBinder, TypingEnv};

for mono_item in discovered_mono_items {
    let MonoItem::Fn(instance) = mono_item else {
        continue;
    };
    if instance.def_id().as_local().is_none() {
        continue;
    }
    let polymorphic_body = ctxt.input.tcx.instance_mir(instance.def);
    let body = ctxt.input.tcx.instantiate_and_normalize_erasing_regions(
        instance.args,
        TypingEnv::fully_monomorphized(),
        EarlyBinder::bind(polymorphic_body.clone()),
    );
    for local in &body.local_decls {
        scan_ty_for_synthesis(ctxt, local.ty)?;
    }
    for block in body.basic_blocks.iter() {
        for statement in &block.statements {
            scan_statement_for_synthesis(ctxt, &body.local_decls, &statement.kind)?;
        }
        if let Some(terminator) = &block.terminator {
            scan_terminator_for_synthesis(ctxt, &body.local_decls, &terminator.kind)?;
        }
    }
}
```

The implementer SHOULD confirm the exact `instantiate_and_normalize_erasing_regions`
API signature against the pinned-nightly rustc (per
`rust-toolchain.toml`); regseq-driver-60's Change A documents
the equivalent call site and exact API forms.

### Change B: no-op for non-generic Instances (regression-guard)

**Anchor**: same.

For a Function Instance whose `instance.args == identity_args`
(non-generic fn called as itself, or any Instance whose args are
all `Param(i)` mapping to themselves), substitution with identity
args returns the body essentially unchanged. The substitution call
is a no-op; the new loop produces the same body view that the
pre-fix loop did.

**Note for the implementer**: do NOT branch on "args is identity"
to skip substitution -- the substitution itself is a no-op in that
case, and the unconditional call is cleaner. Per the
`feedback_no_defensive_fallbacks` rule, only branch when the
spec/semantics demand it; performance micro-optimisation is not a
sufficient reason. Driver-60's Change C made the same observation.

### Change C: harness expectations entry

**Anchor**: `.harness/expectations/regseq-driver-62.md` (new file).

Record:

- Before-fix evidence: cite the canonical repro shape
  (inline in the Description above, re-authored as the B2
  fixture at
  `crates/test-ir-fixtures/fixtures/closure_apply_indirection.rs`) +
  the `BridgeError::Phase4` shape +
  the `bridge/mod.rs:769` polymorphic-body fetch +
  the `bridge/mod.rs:1039-1047` `try_resolve` -> Phase4 conversion
  chain.
- After-fix evidence: the new B2 fixture
  (`crates/test-ir-fixtures/fixtures/closure_apply_indirection.rs`)
  bridges cleanly; `cargo test --workspace` returns to green.
- Out-of-scope: regseq-driver-60's body materializer fix is
  unchanged; the Phase-5 substitution path is independent of the
  Phase-2 pre-scan fix.

## Non-Goals

- Do NOT modify regseq-driver-60's body materializer fix at
  `crates/regseq-driver/src/bridge/body/materialise.rs:287-293`. The
  Phase-5 substitution is correct; this task closes the parallel
  gap in Phase-2.
- Do NOT modify `canonicalize_call_site_callee` at
  `bridge/mod.rs:2441-2482`. The resolver is correct; the bug is
  that the pre-scan was feeding it polymorphic inputs.
- Do NOT modify the `Instance::try_resolve` -> `BridgeError::Phase4`
  conversion at `bridge/mod.rs:1039-1047`. The conversion is the
  correct shape for legitimately-unresolvable call sites; after
  this fix, the pre-scan stops feeding it polymorphic inputs.
- Do NOT change spec.md, pretty-printer.md, or
  generic-c-backend.md. The bug is in the bridge implementation,
  not the spec.
- Do NOT modify `tests/fixtures/playground/`. Playground is the
  user's experimentation space.
- Do NOT retrofit substitution into bridge passes OTHER than the
  Phase-2 synthesis pre-scan and the Phase-5 body materializer
  (driver-60 covers the latter). If other passes prove to have
  the same root-cause shape, file separate tasks; do not bundle.
- Do NOT add a defensive `if args.is_identity { skip_substitute }`
  branch (per `feedback_no_defensive_fallbacks`).

## Test Plan

### B1: existing tests stay green

`cargo test --workspace -- --test-threads=1` green. Specifically:

- regseq-driver-58 / -59 / -60 / -61 tests stay green.
- All existing fixtures in `crates/test-ir-fixtures/fixtures/`
  bridge cleanly. The substitution is a no-op for non-generic
  Instances; existing fixtures have no shape that depended on
  the polymorphic-body view.

### B2: new fixture exercising the bug shape

**New file**:
`crates/test-ir-fixtures/fixtures/closure_apply_indirection.rs`

Source:

```rust
#![no_std]

pub fn apply(x: i32, y: i32, f: impl Fn(i32, i32) -> i32) -> i32 {
    f(x, y)
}

pub fn entry(x: i32, y: i32) -> i32 {
    apply(x, y, |x, y| x + y)
}
```

**New test**: `crates/regseq-driver/tests/bridge_closure_apply_indirection.rs`
(or extend an existing bridge integration test file -- implementer
chooses based on workspace test taxonomy).

Assertions:

- `bridge_fixture("closure_apply_indirection")` succeeds: the helper
  returns a `RegseqModel` without panicking. (`bridge_fixture`
  itself returns `RegseqModel` and panics on `BridgeError` per
  `crates/test-ir-fixtures/src/lib.rs:90-99` and `:254-258`; the
  success path is "did not panic.")
- The resulting `RegseqModel` has at least one Function Instance
  with `flavor == FunctionInstanceFlavor::Item` corresponding to
  the `apply::<{closure}>` monomorphization.
- That Instance's `body.is_some()`.
- The resulting model has at least one
  `SyntheticItemKind::ClosureCall { call_kind: Call, .. }` Item
  (the closure's `Fn::call` shim, synthesised by the bridge per
  spec.md section 13.1 closure-family synthesis (`spec.md:4557-4561`)
  + the `SyntheticItemKind` enum definition at section 13.3
  (`spec.md:4863-4922`) + the eager closure-call synth at
  `bridge/mod.rs:2278+`).

Pre-fix: this test fails because `run_bridge` returns
`Err(BridgeError::Phase4(...))`.

Post-fix: this test passes.

### B3: workspace test suite returns to green

`cargo test --workspace -- --test-threads=1` green (no failing
tests, no panics, no `BridgeError::Phase4` from any committed
fixture).

### B4: end-to-end smoke via `cargo regseq` on the new fixture

After the fix lands, the implementer manually verifies the new
B2 fixture also bridges cleanly via the production wrapper (not
just the in-process test helper):

```bash
# Author a throwaway Cargo crate (or use an existing fixture
# crate setup) whose `src/lib.rs` is the B2 fixture's source.
# Then drive the production wrapper:
PATH="<workspace>/target/debug:$PATH" cargo regseq \
  --target-os linux --platform-file <platform.toml> --emit model
```

Verifies the production-wrapper code path matches the in-process
test-helper code path. Recorded in the harness expectations file
as "manually verified at <commit-sha>". This is OUT-OF-TREE
smoke only; the binding regression guard is B2.

Do NOT use `tests/fixtures/playground/` for this verification --
playground is the user's experimentation space (per
`playground-is-experimentation-space`) and its on-disk state at
any commit is not part of this task's repro contract.

## Review

### Files to send to Codex (`scripts/python/codex_runner.py`)

- This file
  (`.harness/tasks/regseq-driver/62-prescan-args-substitution.md`).

(No spec amendment in this task.)

### Reviewer focus

- **Root-cause framing**: does the Description correctly identify
  `bridge/mod.rs:769` as the polymorphic-body fetch and
  `bridge/mod.rs:1039-1047` as the `try_resolve` ->
  `BridgeError::Phase4` conversion?
- **Same-root-cause-as-driver-60 claim**: confirm by cross-
  referencing driver-60's Change A. The fix shape and API choice
  must match.
- **API correctness**: is
  `tcx.instantiate_and_normalize_erasing_regions(instance.args,
  TypingEnv::fully_monomorphized(),
  EarlyBinder::bind(polymorphic_body.clone()))` the correct
  call shape for the pinned nightly (2026-02-01)? Confirm
  against `rustc_middle::ty::TyCtxt` method tables.
- **`instance_mir(instance.def)` vs `optimized_mir(local_def_id)`**:
  the recommended implementation switches the API call. Confirm
  that `instance_mir` returns the right body for each
  `InstanceKind` (Item -> source body; DropGlue -> shim body;
  VTableShim -> shim body; etc.) per
  `rustc_middle/src/ty/instance.rs` API contract.
- **No defensive fallbacks**: per
  `feedback_no_defensive_fallbacks`. Change B explicitly forbids
  branching on "args is identity" to skip substitution.
- **Citation drift**: every file:line must anchor against HEAD.
- **Scope creep**: per CLAUDE.md. Single-spot production fix +
  one new fixture + harness expectations. Flag any unrequested
  scaffolding.

### Adversarial prompt template

```
<task>
Review .harness/tasks/regseq-driver/62-prescan-args-substitution.md.
Identify:
- Citation drift.
- API correctness against the pinned-nightly rustc.
- Whether the proposed implementation matches driver-60's Change A
  pattern.
- Defensive-fallback smells.
- Scope creep.
</task>

<grounding_rules>
- Cite file:line for every claim.
- Mark hypotheses as hypotheses.
- Don't speculate about code or spec sections you didn't read.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED WITH MINOR FIXES | NOT READY
ROOT-CAUSE FRAMING CHECK: PASS / FAIL with citation
API-CORRECTNESS CHECK: PASS / FAIL with citation
DRIVER-60-PARITY CHECK: PASS / FAIL with citation
CITATION DRIFT: list or "none"
NO-DEFENSIVE-FALLBACK AUDIT: pass/fail
SCOPE-CREEP CHECK: pass/fail with citation
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
- `cargo test --workspace -- --test-threads=1` green. Specifically:
  - The new B2 closure-apply-indirection fixture test passes.
  - No regressions in regseq-driver-58 / -59 / -60 / -61 tests
    or any existing workspace test.
- `git diff --stat HEAD..` shows changes confined to:
  - `crates/regseq-driver/src/bridge/mod.rs` (Change A: the
    pre-scan loop)
  - new fixture file
    `crates/test-ir-fixtures/fixtures/closure_apply_indirection.rs`
  - new test file under `crates/regseq-driver/tests/` (or
    extension of an existing test file)
  - new `.harness/expectations/regseq-driver-62.md` (Change C)
- No spec.md, pretty-printer.md, generic-c-backend.md, or
  validator changes.
- Manual verification via `cargo regseq` on the B2 fixture per B4.
- Codex Task Review verdict: APPROVED or APPROVED WITH MINOR FIXES.

## Acceptance Criteria

1. **Pre-scan substitutes Instance args**:
   `crates/regseq-driver/src/bridge/mod.rs:769` (or immediately
   after the existing body fetch) applies `instance.args` to the
   body via `tcx.instantiate_and_normalize_erasing_regions` (or
   the equivalent `Instance` method).

2. **`instance_mir(instance.def)` replaces `optimized_mir(local_def_id)`**:
   the body fetch uses `instance_mir` so non-`Item` flavors
   (DropGlue, VTableShim, etc.) receive the right body view per
   the pinned-nightly rustc API contract.

3. **Pre-fix repro stops failing**: the canonical
   closure-apply-indirection shape (the B2 fixture below)
   compiles cleanly via both the `bridge_fixture` test helper
   and the production `cargo regseq` wrapper.

4. **B2 fixture test passes**: the new
   `closure_apply_indirection.rs` fixture and its integration
   test return `Ok(_)` with a Function Instance carrying
   `body.is_some()` and a `SyntheticItemKind::ClosureCall` Item
   present in the model.

5. **B1/B3 no-regression**: every existing test in the workspace
   stays green.

6. **No defensive fallback for identity-args**: Change A does
   NOT branch on "args is identity" to skip the substitution.
   Identity substitution is applied unconditionally; the rustc
   folder makes it a no-op naturally.

7. **No spec / pretty-printer / validator changes**: `git diff
   main -- docs/ crates/regseq-model/src/` returns zero.

8. **Expectations file lands**:
   `.harness/expectations/regseq-driver-62.md` records before /
   after evidence with file:line citations + the manual
   `cargo regseq` smoke-verification SHA from B4.
