# regseq-driver-73 — Accept `GlobalAlloc::Function` provenance in const translation

> Plan: `.harness/audits/2026-05-23-r1-const-fnptr-provenance-plan.md`
> (Codex-reviewed; hidden-blocker check cleared — rank-0 Function relocs
> are an already-validated construct). This is the "R1" item surfaced by
> the C2/B2 probes. (`regseq-driver-73` was reserved in the retired C1
> plan's title; C1 was never filed, so the number is free.)

## 1. Description

The bridge's const translation rejects any pointer whose provenance is
`GlobalAlloc::Function { instance }` — a function pointer baked into a
`const` or `static`. This task teaches both const-provenance sites to
**accept** that case by materializing the target function instance and
emitting a `Const::Reloc` / `Byte::Reloc` to it.

This **covers matrix row FF-004** (`FunctionInstanceFlavor::ReifyShim`):
a `#[track_caller]` function reified into a `static` carries
`Function { ReifyShim(target) }`, so materializing it yields a
`ReifyShim`-flavored instance. It also **closes the C2-logged gap**
(fn pointers embedded in consts/statics — plain-fn statics, fn-ptr
arrays, fn-ptr consts — currently spuriously rejected).

It is a bounded bridge fix: no new validator concepts (rank-0 Function
relocs are already representable and validated), reusing the existing
instance-materialization machinery.

## 2. Input Context

### Design source
- `.harness/audits/2026-05-23-r1-const-fnptr-provenance-plan.md` (this
  task's plan, with the verified mechanism + risks).
- `.harness/audits/2026-05-23-c2-fnptraddrshim-retirement.md` and
  `.harness/audits/2026-05-23-b2-deferred-shim-probe.md` — the probes
  that surfaced this gap and mapped the shape surface.

### Spec anchors
- `docs/discussions/regseq-model/spec.md` §9.7 — `Const::Reloc`
  semantics (relocation to an instance).
- `docs/discussions/regseq-model/bridge-coverage-matrix.md` — FF-004 row
  (`## 2.3` section) and the matrix legend.

### Bridge code anchors (consumed/modified)
- **Rejection sites (modify):** `crates/regseq-driver/src/bridge/const_.rs:414-431`
  (`translate_scalar`, `Scalar::Ptr`) and `:526-542` (`allocation_bytes`).
- **Reloc builders (model for the new helper):** `const_.rs:567-585`
  (`instance_ref_for_const` / `instance_ref_for_static`).
- **Validator (the materialization constraint):** `crates/regseq-model/src/model.rs:3250-3272`
  (`validate_const_references`), `:3381-3395` (`check_instance_ref` — the
  reject), `:3373-3405` (`instance_refs` / `instance_ref`), `:4060-4069`
  (`function_instance_disambig` = `rustc_instance_kind_sort_rank(flavor)`
  for Function instances).
- **Materialization precedent (reuse):** `crates/regseq-driver/src/bridge/body/terminator.rs:413-449`
  (`bind_closure_once_shim_instance`) and `:729` (`lookup_or_create_direct_instance`,
  private — see Change C; it self-drains `pending_items` at `:747`).
- **Flavor mapping:** `crates/regseq-driver/src/bridge/mod.rs:2506-2529`
  (`function_instance_flavor_for_kind`).
- **Phase-5 const/static init entry points (new call-site context):**
  `mod.rs:1350` (source statics/consts), `:1793` (`translate_static_init`),
  `:1860` (`translate_const_item`); F4-2 instance sort at `:641`.
- **Body-absent shim handling:** `crates/regseq-driver/src/bridge/body/materialise.rs:246-269`
  (`ReifyShim` etc. → body `None`).
- **Public ref helpers:** `crates/regseq-model/src/instance.rs:170`
  (`regseq_instance_kind_rank`, pub) and `:234`
  (`rustc_instance_kind_sort_rank`, pub); `instance_ref.rs:10-19`
  (`InstanceRef`).

### Prior tasks
- **regseq-driver-72** — sub-task B materialized a body-absent shim
  flavor (`ClosureOnceShim`) via `lookup_or_create_direct_instance`; the
  direct precedent for Change A.
- **regseq-driver-70 (F70)** — RustCall ABI signature-translation
  rejection at `bridge/fnsig.rs`; the signature hazard to watch (Risk R-3).
- **B2 / F1 (2026-05-23)** — classified FF-004 `blocked-open-bridge-bug`
  with this fix as the recorded unblock.

### Test surface
- `crates/regseq-model/tests/id_referential_integrity_validator.rs:273-287`
  (`invariant_9_pass_const_reloc_and_bytes_reloc`) — proves rank-0
  Function `Const::Reloc` + `Byte::Reloc` validate OK.
- `crates/test-ir-fixtures` (bridge-side unit tests) and the
  bridge-coverage-runner sidecar + Mode A.

## 3. Implementation Notes

### Change A — shared Function-provenance helper
Add one helper (in `const_.rs`, or a small shared bridge module if both
sites import it more cleanly) that takes the rustc `Instance` from
`GlobalAlloc::Function { instance }` and returns an `InstanceRef`:
1. `callee_ty = ty::Ty::new_fn_def(tcx, instance.def_id(), instance.args)`.
2. `translate_ty(cx, callee_ty)` → resolve to `Ty::FnDef(item, model_args)`.
3. `flavor = function_instance_flavor_for_kind(instance.def)`
   (`Item` for plain fns, `ReifyShim` for `#[track_caller]`).
4. `make_fn_sig` = translate the instance's post-mono fn signature:
   `tcx.fn_sig(instance.def_id()).instantiate(tcx, instance.args)` →
   `fnsig::translate_fn_sig` (public at `bridge/fnsig.rs:23-26`), the same
   recipe the terminator path uses (`terminator.rs:721-726`).
5. `id = lookup_or_create_direct_instance(cx, item, model_args, flavor,
   None, make_fn_sig)` — materializes + dedups the Function instance.
6. Obtain the matching `InstanceRef` via the canonical model computation
   (Change D's `instance_ref_of(id)`) — **not** by local
   hand-reconstruction.

Mirror `bind_closure_once_shim_instance` (terminator.rs:433-448) for
steps 1-5.

### Change B — wire both sites
- `translate_scalar` (`const_.rs:427`): add an explicit
  `GlobalAlloc::Function { .. }` arm → `Const::Reloc(helper(...)?)`.
- `allocation_bytes` (`const_.rs:538`): add an explicit
  `GlobalAlloc::Function { .. }` arm → `Byte::Reloc(helper(...)?)`.
- Keep the `_` arm rejecting everything else. **`GlobalAlloc::VTable`
  (and any other variant) MUST stay rejected** — no scope creep.

### Change C — visibility
Make `lookup_or_create_direct_instance` `pub(crate)` (or relocate the
shared materialization into a module both `body/terminator.rs` and
`const_.rs` import). Mechanical; no behavior change.

### Change D — canonical InstanceRef API (drift-proofing)
Add `pub fn instance_ref_of(&self, id: InstanceId) -> InstanceRef` to
`RegseqModel`, exposing the existing private `instance_ref(&Instance)`
computation (`model.rs:3397-3405`). Change A reads the ref back from the
materialized instance through it, so the bridge's `InstanceRef` cannot
drift from the validator's. (Hand-building is technically possible — all
inputs are public — but the API removes the drift risk; see Risk R-1.)

## 4. Non-Goals

- `GlobalAlloc::VTable` provenance (separate task class).
- Broad const-generic argument support (deferred driver-37 / retired C1).
- The runtime-reify path's missing `resolve_for_fn_ptr` adjustment
  (runtime `let fp: fn() = tc_fn` resolves to `Item`, not `ReifyShim`) —
  NOT required to cover FF-004; the static path is the chosen unblock.
- R2 (tuple-clone `ItemMissingInstance`) and R3 (closure-clone ICE).
- Any change to the rejection wording or behavior for non-`Function`
  provenance.

## 5. Test Plan

(All bridge runs use the standard `#![no_std]`, panic=abort,
mir_opt_level=0 config.)

- **B1 (FF-004, the covered row):** `#[track_caller] pub fn t() {} pub
  static FP: fn() = t;` bridges OK; the model contains a
  `ReifyShim`-flavored Function instance; `validate()` passes. Author the
  `FF-004.rs` snippet + sidecar row (`inspect = { kind =
  "FunctionInstanceFlavor", arg = "ReifyShim" }`); reclassify the matrix
  row `blocked-open-bridge-bug` → `covered`.
- **B2 (`Const::Reloc` scalar, top-level static):** `pub fn t() {} pub
  static FP: fn() = t;` bridges OK; `Item`-flavored instance materialized;
  `validate()` passes.
- **B3 (`Const::Reloc` scalar, top-level const):** `pub fn t() {} pub
  const FP: fn() = t; pub fn u() -> fn() { FP }` bridges OK (locks the
  R-2 const-initializer phase path).
- **B4 (`Byte::Reloc` allocation path):** `pub fn a() {} pub fn b() {}
  pub static T: [fn(); 2] = [a, b];` bridges OK; both instances
  materialized; `validate()` passes.
- **B5 (negative, no scope creep):** `GlobalAlloc::VTable` provenance is
  still rejected with the existing `UnsupportedConst` wording. Use an
  **aggregate** static so the pointer routes through `allocation_bytes`
  (`:538`) rather than the scalar-ref path (a bare `static D: &dyn Trait`
  is classified scalar at `const_.rs:277-305` and goes through the scalar
  read, not `allocation_bytes`) — e.g. a tuple/array static embedding a
  `&dyn Trait`. If a minimal source shape proves flaky, fall back to a
  targeted unit test asserting the `_` arm at both `:427-431` and
  `:538-542` still rejects non-`Function` variants.
- **B6 (dedup):** a fn used BOTH in a const fn-ptr AND called directly
  converges on a single instance (no duplicate) — assert instance count.
- **B7 (regression):** Mode A `Total = Pass`, `Fail = 0`, with the FF-004
  row added (143 expected); the 142 pre-existing rows unaffected.
  `cargo build` / `cargo fmt --all -- --check` /
  `cargo clippy -p regseq-driver -p regseq-model -p test-ir-fixtures
  --all-targets -- -D warnings` / `cargo test` clean.

## 6. Review

### Files to send to Codex
- `crates/regseq-driver/src/bridge/const_.rs` (the two arms + helper)
- the new/changed model API (`crates/regseq-model/src/model.rs`)
- `crates/regseq-driver/src/bridge/body/terminator.rs` (visibility)
- the new tests + `FF-004.rs` snippet + `rows.toml` + matrix diff
- this spec + the R1 plan doc

### Reviewer focus
1. **InstanceRef match (R-1):** does Change A's ref EXACTLY equal what
   `validate()` computes? Is `instance_ref_of` wired so a mismatch is
   impossible?
2. **Phase ordering (R-2):** is `lookup_or_create_direct_instance` safe
   from the top-level static AND const initializer context (B2/B3)? Any
   `pending_items` / F4-2 sort hazard?
3. **Scope:** is `VTable` (and every non-`Function` variant) still
   rejected? No widening of the `_` arm.
4. **Signature hazards (R-3):** does the `make_fn_sig` path reject any
   in-scope shape (cf. F70 RustCall ABI)?
5. **Dedup (R-5):** does the const-materialized instance converge with an
   eagerly-constructed / call-site instance for the same fn?
6. Diff confinement per the Quality Gate.

### Adversarial prompt template (four-block)
```
<task>
Review the regseq-driver-73 implementation (accept GlobalAlloc::Function
provenance in const translation). Verify: (1) the bridge-built InstanceRef
for a const Function reloc exactly matches the validator's instance_ref()
so validate() cannot fail on a match basis; (2) lookup_or_create_direct_instance
is safe from top-level static AND const initializer contexts; (3) GlobalAlloc::VTable
and all non-Function provenance remain rejected; (4) FF-004 (ReifyShim) is
covered and Mode A is green; (5) the diff is confined to the const-provenance
sites + the shared helper + the model API + terminator visibility + tests.
</task>
<grounding_rules>
Cite file:line for every claim. Reproduce the FF-004 + plain-fn-static +
fn-ptr-array bridge runs. Mark hypotheses as hypotheses.
</grounding_rules>
<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
<per-focus PASS/FAIL with citation>
NEW ISSUES: numbered or "none"
REQUIRED FIXES: numbered (only if VERDICT != APPROVED)
</structured_output_contract>
<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## 7. Quality Gate

- `cargo build` clean; `cargo fmt --all -- --check` clean;
  `cargo clippy -p regseq-driver -p regseq-model -p test-ir-fixtures
  --all-targets -- -D warnings` clean; `cargo test` green
  (`RUST_TEST_THREADS=1` if needed).
- Mode A: `Total = Pass`, `Fail = 0`, exit 0.
- Diff confined to: `bridge/const_.rs` (+ a shared helper location),
  `regseq-model` (the `instance_ref_of` API), `bridge/body/terminator.rs`
  (visibility only), the new tests, `FF-004.rs`, `rows.toml`, and the
  FF-004 matrix-row reclassification. No unrelated bridge changes; no
  changes to non-`Function` provenance behavior; no edits to the F0
  helper surface.

## 8. Acceptance Criteria

1. `#[track_caller]` static reify bridges OK and the model carries a
   `ReifyShim`-flavored Function instance; `validate()` passes (B1).
2. Plain-fn `static` (B2), fn-ptr `const` (B3), and fn-ptr array `static`
   (B4) all bridge OK (previously `UnsupportedConst`); `validate()`
   passes; the materialized instances are deduped (B6).
3. `GlobalAlloc::VTable` and all other non-`Function` provenance remain
   rejected with the existing `UnsupportedConst` wording (B5).
4. The bridge-emitted `InstanceRef` for every const Function reloc
   matches the validator's canonical `instance_ref()` (obtained via the
   `instance_ref_of` API, not hand-built).
5. FF-004 matrix row is `covered` with a committed `FF-004.rs` snippet +
   sidecar row; Mode A reports `Total = Pass`, `Fail = 0` (143 rows);
   the 142 pre-existing rows are unaffected.
6. `cargo build` / `fmt` / `clippy -D warnings` / `cargo test` all clean.
7. Diff confined per the Quality Gate; `GlobalAlloc::VTable` and broad
   const-generic support are untouched (Non-Goals respected).
