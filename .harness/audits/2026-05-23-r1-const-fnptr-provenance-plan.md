# R1 plan: accept `GlobalAlloc::Function` provenance in const translation

**Status**: DRAFT for Codex review (2026-05-23). Feasibility probe
GREEN (see §1). First *real bridge fix* of the bridge-coverage
workstream — goes through the full task flow (plan → Codex → spec →
Codex → file → implementer), not the direct author-and-commit pattern
used for the zero-change row covers (FF-012/FF-013).

**Goal**: teach the bridge's const translation to accept a pointer
whose provenance is `GlobalAlloc::Function { instance }` (a fn pointer
baked into a `const`/`static`), emitting a `Const::Reloc` / `Byte::Reloc`
to the materialized function instance. This **covers FF-004** (ReifyShim,
via a `#[track_caller]` static reify) and **closes the C2-logged gap**
(fn pointers embedded in consts/statics, currently spuriously rejected).

---

## 1. Problem statement (verified at HEAD on pinned nightly-2026-02-01)

The const translator handles pointer provenance at two sites, each
accepting only `GlobalAlloc::Memory` and `GlobalAlloc::Static` and
rejecting everything else (including `Function`) in a `_` arm:

- `crates/regseq-driver/src/bridge/const_.rs:414-431` — `translate_scalar`,
  `Scalar::Ptr` path → rejects with `UnsupportedConst { const_repr:
  "pointer scalar over {alloc:?}" }` (`:428-430`).
- `crates/regseq-driver/src/bridge/const_.rs:526-542` — `allocation_bytes`
  (pointer-sized provenance slot) → rejects with `UnsupportedConst {
  const_repr: "allocation provenance over {alloc:?}" }` (`:539-541`).

**Empirically mapped shape surface** (in-process bridge runs):

| Source shape | Site | Carried rustc instance |
| ------------ | ---- | ---------------------- |
| `static FP: fn() = plain_fn` | `:429` | `Function { Item(t) }` |
| `const FP: fn() = plain_fn` | `:429` | `Function { Item(t) }` |
| `static T: [fn(); 2] = [a, b]` | `:540` | `Function { Item(a) }` |
| `static FP: fn() = track_caller_fn` | `:429` | `Function { ReifyShim(t) }` |

**The validator requires materialization.** `RegseqModel::validate`
(`crates/regseq-model/src/model.rs:3240-3242`, `:3373-3405`) builds
`instance_refs` from *all materialized* `model.instances` (via the
private `instance_ref()` at `:3397-3405`) and, in
`validate_const_references` (`:3250-3272`), rejects any `Const::Reloc` /
`Byte::Reloc` whose `InstanceRef` is not in that set
(`check_instance_ref`, `:3381-3390`, `CONST_RELOC_INSTANCE`). The
existing `Memory`/`Static` arms therefore *materialize their pointee
first* — `synthesise_const_memory_static(cx, pointee)` at `:421` / `:532`
for the `Memory` case; statics are materialized as instances elsewhere.
So the R1 fix is **not** a one-line "accept the alloc": it must
materialize the target function instance so its `InstanceRef` is in the
validated set.

**What R1 unblocks:**
- **FF-004 (`ReifyShim`)** — currently `blocked-open-bridge-bug`. A
  `#[track_caller]` static reify carries `Function { ReifyShim(t) }`;
  materializing it yields a `ReifyShim`-flavored Function instance,
  which the inspect predicate `FunctionInstanceFlavor=ReifyShim`
  matches → `covered`.
- **The C2-logged gap** — plain-fn statics, fn-ptr arrays, and fn-ptr
  consts stop being spuriously rejected (correctness/completeness; not a
  single new flavor row).

## 2. Code-surface grounding

- Rejection sites: `const_.rs:414-431`, `:526-542`.
- Reloc machinery: `Const::Reloc(InstanceRef)` / `Byte::Reloc(InstanceRef)`
  (`crates/regseq-model/src/const_value.rs:50,75`); bridge builders
  `instance_ref_for_const` (`const_.rs:567-576`) and
  `instance_ref_for_static` (`const_.rs:578-585`) build refs *by hand*
  (`regseq_kind_rank: 1`, def_path_hash, EMPTY args, disambig 0).
- `InstanceRef` shape: `crates/regseq-model/src/instance_ref.rs:10-19`
  (rank + def_path_hash + args_key + fn_shim_disambig).
- Validator canonical ref: `model.rs:3397-3405` `instance_ref()` =
  `regseq_instance_kind_rank(kind)` + `item_def_paths[item]` +
  `args_key_for_generic_args(...)` + `function_instance_disambig(kind)`.
  `function_instance_disambig` for a Function instance is simply
  `rustc_instance_kind_sort_rank(flavor)` (`model.rs:4060-4062`), and
  BOTH `regseq_instance_kind_rank` (`instance.rs:170`) and
  `rustc_instance_kind_sort_rank` (`instance.rs:234`) are `pub`, as are
  the args-key helpers (`args_key.rs:117`) — so the bridge *can*
  hand-build the ref. But `function_instance_disambig` and
  `instance_ref()` themselves are private, so reusing the canonical
  computation (Change D) is preferred to avoid drift if they ever
  diverge — see Risk R-1.
- **Hidden-blocker check (cleared):** a rank-0 Function `Const::Reloc` /
  `Byte::Reloc` is already a representable, *validated* construct — the
  test `invariant_9_pass_const_reloc_and_bytes_reloc`
  (`crates/regseq-model/tests/id_referential_integrity_validator.rs:273-287`)
  constructs rank-0 Function relocs and asserts `validate() == Ok(())`,
  and the pretty-printer has explicit Function-reloc arms
  (`pretty.rs:1158`). So no validator invariant forbids R1's output;
  the only constraint is that the referenced instance be materialized
  (Claim 2 / Change A).
- Materialization precedent (the model for Change A): F72 sub-task B
  `bind_closure_once_shim_instance`
  (`crates/regseq-driver/src/bridge/body/terminator.rs:413-449`)
  materializes a function instance from a rustc identity:
  `new_fn_def(def_id, args)` → `translate_ty` →
  `Ty::FnDef(item, model_args)` → `lookup_or_create_direct_instance(cx,
  item, args, flavor, None, make_fn_sig)` → `InstanceId`, with a
  **body-absent shim flavor** (exactly ReifyShim's case; body stays
  `None` per `materialise.rs:246-269`).
- `function_instance_flavor_for_kind` (`mod.rs:2506-2529`) maps a rustc
  `InstanceKind` → `FunctionInstanceFlavor` (`ReifyShim(_, _) =>
  ReifyShim`, `Item(_) => Item`, ...).
- `lookup_or_create_direct_instance`
  (`crates/regseq-driver/src/bridge/body/terminator.rs:729`) is
  **private** to terminator.rs — see Change C.
- `GlobalAlloc::Function { instance }` is a struct variant carrying the
  rustc `Instance` (rustc-src `mir/interpret/mod.rs:120,294`).

## 3. Proposed scope (changes)

**Change A — shared Function-provenance helper.** Add one helper (in
`const_.rs`, or a shared bridge module) that takes the rustc `Instance`
from `GlobalAlloc::Function { instance }` and returns an `InstanceRef`:
1. `callee_ty = ty::Ty::new_fn_def(tcx, instance.def_id(), instance.args)`.
2. `translate_ty(cx, callee_ty)` → resolve to `Ty::FnDef(item, model_args)`.
3. `flavor = function_instance_flavor_for_kind(instance.def)`.
4. `make_fn_sig` = translate the instance's fn signature (post-mono).
5. `id = lookup_or_create_direct_instance(cx, item, model_args, flavor,
   None, make_fn_sig)` — materializes (and dedups) the Function instance.
6. Return the `InstanceRef` by reading it back from the materialized
   instance via the canonical model computation (Change D's
   `instance_ref_of(id)`) — **not** by local hand-reconstruction (avoids
   drift; Risk R-1).

**Change B — wire both sites.** In `translate_scalar` (`:427`) and
`allocation_bytes` (`:538`), replace the `_`-arm rejection for the
`Function` case with `Const::Reloc(helper(...)?)` / `Byte::Reloc(helper(...)?)`.
**Leave `GlobalAlloc::VTable` (and any other variant) rejected** — match
`GlobalAlloc::Function { .. }` explicitly, keep a `_` reject arm.

**Change C — visibility.** Make `lookup_or_create_direct_instance`
`pub(crate)` (or relocate the shared materialization into a module both
`terminator.rs` and `const_.rs` import). Minimal, mechanical.

**Change D — InstanceRef matching (avoid drift).** The bridge must
produce the *exact* `InstanceRef` the validator computes for the
materialized instance, or `check_instance_ref` fails. The bridge *could*
hand-build it today (all inputs are public: `regseq_instance_kind_rank`,
`rustc_instance_kind_sort_rank`, the args-key helpers) — that is how the
existing `instance_ref_for_static` works. But to eliminate the drift
risk (if `function_instance_disambig`'s mapping ever changes), the
preferred design is a small model API `pub fn instance_ref_of(&self, id:
InstanceId) -> InstanceRef` exposing the existing private `instance_ref()`
computation, which Change A reads back. This is preferred, not mandated
by visibility.

## 4. Snippet / coverage requirements

- **FF-004.rs** — author a `#[track_caller]` static reify snippet
  (`#[track_caller] fn t() {} static FP: fn() = t;` plus a `pub` use so
  it is retained). Reclassify FF-004 `blocked-open-bridge-bug` →
  `covered`; add the sidecar row (`inspect = { kind =
  "FunctionInstanceFlavor", arg = "ReifyShim" }`).
- **Bridge regression tests** for the C2-gap shapes that produce no new
  *flavor* row but must now bridge OK. Cover BOTH reloc paths and BOTH
  const-context entry points:
  - `Const::Reloc` scalar path (`:427`): a plain-fn `static FP: fn() = t`
    AND a fn-ptr `const FP: fn() = t` (top-level static + const, per R-2).
  - `Byte::Reloc` allocation path (`:540`): a fn-ptr array
    `static T: [fn(); 2] = [a, b]`.
  - The `#[track_caller]` ReifyShim case (the FF-004 snippet) doubles as
    the scalar-path ReifyShim-flavor regression.
  Lean: `test-ir-fixtures` bridge-side unit tests (they assert "no longer
  rejected" + the expected flavor), with FF-004 as the one sidecar/matrix
  row. Decide final placement at spec time.
- Run Mode A: expect Total/Pass to rise by the FF-004 row, Fail = 0.

## 5. Acceptance criteria (preview)

1. `static FP: fn() = track_caller_fn` bridges OK and the model contains
   a `ReifyShim`-flavored Function instance; `validate()` passes.
2. Plain-fn `static`/`const` and fn-ptr-array `static` bridge OK
   (previously `UnsupportedConst`).
3. `GlobalAlloc::VTable` provenance is still rejected (no scope creep).
4. FF-004 row `covered`; Mode A green; no regression in the 142 existing
   sidecar rows.
5. `cargo build` / `fmt` / `clippy -D warnings` / `cargo test` clean.

## 6. Out of scope

- `GlobalAlloc::VTable` provenance (separate concern).
- Broad const-generic support (deferred driver-37 / retired C1).
- R2 (tuple-clone `ItemMissingInstance`) and R3 (closure-clone ICE).
- The runtime-reify path's missing `resolve_for_fn_ptr` adjustment
  (FF-004's *other* path — the static path is the chosen unblock; the
  runtime path's track_caller→ReifyShim adjustment is a separate, larger
  question and is NOT required to cover FF-004).

## 7. Risks

- **R-1: InstanceRef mismatch (drift).** A bridge-built ref that diverges
  from the validator's `instance_ref()` fails `check_instance_ref`. Not a
  hard blocker (the inputs are public and hand-buildable), but the
  mitigation is Change D (read the ref back from the materialized
  instance via a canonical model API) so the two computations cannot
  drift.
- **R-2: const-context materialization / phase ordering (bounded).**
  Codex traced the ordering: top-level `static`/`const` initializer
  translation runs in Phase 5 (`mod.rs:1350` constructs source
  statics/consts; `translate_static_init` `mod.rs:1793`,
  `translate_const_item` `mod.rs:1860`); `lookup_or_create_direct_instance`
  drains its own `pending_items` (`body/terminator.rs:747`) and the final
  F4-2 instance sort (`mod.rs:641`) absorbs push-order interleaving. So
  the risk is bounded, not a blocker — but this is a NEW call site, so the
  spec MUST add regression coverage for BOTH a top-level `static`
  initializer and a top-level `const` initializer (in addition to the
  body-operand path), to lock the phase behavior.
- **R-3: signature translation hazards.** Materializing requires a fn
  sig; exotic ABIs could reject (cf. F70's RustCall ABI gap at
  `fnsig.rs`). Plain `fn()` is fine; out-of-scope ABIs should escalate,
  not crash.
- **R-4: recursion/cycles.** A const fn-ptr whose target is reachable
  from another const. The `Memory` arm already self-recurses
  (`allocation_bytes`); dedup via the `(ItemId, ArgsKey, flavor)` cache
  in `lookup_or_create_direct_instance` should bound it. Confirm.
- **R-5: dedup with eager instances.** If the same fn is also called
  directly elsewhere, the cache must converge on one instance (same
  pattern F72 sub-task B relied on).

## 8. Open questions for Codex

1. Is adding `pub fn instance_ref_of(&self, InstanceId) -> InstanceRef`
   to the model the right way to guarantee the bridge/validator ref
   match (Change D), or is there an existing exposed path?
2. Is `lookup_or_create_direct_instance` safe to call while translating a
   **top-level `static`/`const` initializer** (vs only mid-body)? Any
   Phase-2/Phase-5 ordering hazard with `pending_items`/handles?
3. Should the C2-gap regression shapes (plain-fn static, fn-ptr array,
   fn-ptr const) be sidecar/matrix rows or bridge unit tests?
4. Any reason to prefer materializing the **target** of a `ReifyShim`
   (the underlying `Item`) instead of the `ReifyShim` itself? (The probe
   shows rustc hands us the `ReifyShim` instance directly; materializing
   it as-is is what yields FF-004 coverage.)
