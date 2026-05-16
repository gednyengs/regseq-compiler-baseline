# Verification Audit — regseq-driver-{37,38,39,40}

**Date:** 2026-05-15
**Auditor:** Claude Opus 4.7
**Method:** Line-by-line spec-coverage audit against the v2/v4 task specs, the locked DSL spec (`docs/discussions/regseq-model/spec.md`, `docs/discussions/regseq-model/pretty-printer.md`), and the pinned rustc source at `~/.rustup/toolchains/nightly-2026-02-01-.../rustc-src/`.
**Scope:** Only the four tasks that have been re-executed under v2/v4 specs since the 2026-05-15 reset. Tasks 42 and 43 are pending re-execution and were skipped per user instruction.

---

## At-a-glance

| Task | Verdict   | HIGH | MEDIUM | LOW | Build/Tests |
| ---- | --------- | ---- | ------ | --- | ----------- |
| 37   | **PARTIAL** | 2  | 2      | 3   | green       |
| 38   | **PASS**    | 0  | 0      | 1   | green       |
| 39   | **PASS**    | 0  | 0      | 3   | green       |
| 40   | **PASS\*** | 2  | 1      | 2   | green       |

\* Driver-40's HIGH findings are coordination defects with driver-37 (item-table routing) and the not-yet-re-executed driver-42 (F4-2 ordering); the driver-40 deliverables themselves are spec-conforming. The HIGH findings will surface as model-validation failures once driver-42 lands and a workspace validator runs end-to-end.

**Cross-cutting headline.** The most material defect is a single line: `bridge/ty.rs:355` (`let item_id = cx.model.items.push(item);` in `intern_item`). This violates driver-37 audit-finding F4 ("No direct push to `model.items` from Phase 2"); it cascades into a duplicate-Item bug for closures (driver-40 HIGH #1) and undermines F4 determinism for every source-derived ADT (Struct/Enum/Union/FnDef/Closure).

---

## 1. regseq-driver-37 — TyKind Translation and Rejections

**Verdict: PARTIAL** — §15.0 dispatch table, §15.1 rejection rows (including Appendix A.1), special-ADT dispatch ordering, all six NonRigidTyKind variants, custom-auto-trait/DynAutoOnly/DynStar/Coroutine/Pat/UnsafeBinder/Foreign rejections, the parallel `Ty::Closure`/`Ty::Struct` view (F2), and the VTableType-backed `Ty::Dynamic.vtable_ty` (F3) are all implemented and exercised. **Two HIGH-severity audit-finding refinements are unimplemented**, and the substantive-HRTB predicate is a string-matching heuristic rather than a structural Ty visitor.

### Findings

#### HIGH-37-1 — `intern_item` bypasses `pending_items` (audit-finding F4)
- **File:** `crates/regseq-driver/src/bridge/ty.rs:311-360` (push at `:355-358`)
- **Spec:** task-spec lines 325-330 / spec.md §3.4 D-12-5.3.
- **Symptom:** Every source-derived ADT — every Struct, Enum, Union, FnDef, and Closure captures-struct — is appended to `cx.model.items` in rustc-collector traversal order, with `SyntheticItemKind::None` and a direct push of `item_def_paths` / `item_synthetic_kinds` / `domain_attrs.item_attrs`. The HandleTable is never consulted. ItemId assignment is therefore not F4-deterministic for source-derived items; only synth items (vtable + closure-call + drop-glue + ConstMemoryStatic) get the F4 sort treatment.
- **Recommendation:** Refactor `intern_item` to allocate an `ItemHandle` via `cx.handles.fresh_handle()`, push a `PendingItem`, and return the handle (or resolve via Phase-4). All call-sites in `translate_erased_ty` need to consume the handle and resolve at Phase 4 (or after `assign_pending_items` runs). Mirror the pattern used by `synth/closure.rs::push_closure_captures_item` (`synth/closure.rs:143-157`).

#### HIGH-37-2 — Closure-call shim rewrite not wired at terminator site (audit-finding F1c)
- **File:** `crates/regseq-driver/src/bridge/body/terminator.rs` (nothing matches `LangItem::Fn`/`FnMut`/`FnOnce`/`ClosureCall` — verified by `grep`).
- **Spec:** task-spec lines 286-299 / spec.md §13.1 (lines 4556-4566).
- **Symptom:** The FnPtr-side un-tupling at `bridge/ty.rs:117-135` handles the type-level RustCall → Rust ABI mapping for reified fn-pointers, but the body-level terminator bridge does not recognise direct `Fn::call` / `FnMut::call_mut` / `FnOnce::call_once` calls against a closure-captures argument and substitute the callee for the synthesized `ClosureCall` Function Item. Result: at runtime (post-Phase-5 body materialisation, which is driver-43's concern but consumes this rewrite), the terminator's callee `Operand::Constant` still points at the RustCall-ABI trait method and the args tuple is not un-packaged. The pre-existing reference test `bridge_body::translates_representative_place_and_rvalue_rows_from_mir` does not exercise this path — it inspects statements via `translate_fn_statements`, not the call-terminator.
- **Recommendation:** In `translate_call_callee` (`bridge/body/terminator.rs:128-160`), add a closure-trait-method recognition step: if the callee's `DefId` resolves to `LangItem::Fn|FnMut|FnOnce`'s `call|call_mut|call_once` method AND the first generic arg is a closure type that has a cached `ClosureCall` synth handle in `cx.closure_synth_cache`, substitute the callee `Operand::Constant`'s `FnDef` to the synth shim and untuple the trailing tupled-args slot. Add a corresponding terminator-level integration test.

#### MEDIUM-37-3 — Substantive-HRTB predicate is string-matching
- **File:** `crates/regseq-driver/src/bridge/ty.rs:778-807` (`has_substantive_hrtb`).
- **Spec:** spec.md §17.1 lines 6233-6265 ("walking the body and identifying every `TyKind::Ref` node whose lifetime slot binds a region").
- **Symptom:** The predicate looks for substrings (`"for<"`, `"<&'"`, `"fn((&'"`, `"*const &'"`, `"*mut &'"`, `" as "`, `"-> &"`) and counts binder-name occurrences in `ty.to_string()`. It satisfies the seven worked examples encoded in spec.md:6266-6280 (and in the in-file unit tests at `bridge/ty.rs:832-845`), but it is fragile against any pretty-printer drift or non-canonical rendering. A rustc API bump that re-spaces Display output, or any unusual identifier containing `for<` or ` as `, will produce silent false positives/negatives.
- **Recommendation:** Replace with a `TypeVisitor` over `Binder::skip_binder()` that counts `ReBound` occurrences per immediate-input-ref-slot position and enforces the five spec conditions structurally. The current heuristic should remain as a fallback only if there's an explicit decision to defer; otherwise it is technical debt.

#### MEDIUM-37-4 — `Ty::Dynamic.vtable_ty` slot count uses approximation, not `tcx.vtable_entries`
- **File:** `crates/regseq-driver/src/bridge/ty.rs:617-652` (`vtable_entry_count`).
- **Spec:** task-spec lines 317-318 ("slot count from `tcx.vtable_entries(principal_poly_trait_ref)`").
- **Symptom:** `tcx.vtable_entries(poly_trait_ref)` requires a fully-resolved trait ref with a self type, which is unavailable at `Ty::Dynamic` translation time (verified at `~/.rustup/.../rustc_middle/src/ty/vtable.rs:53-75` — rustc itself uses `vtable_min_entries` for this case). The implementation mirrors rustc's `vtable_min_entries` (3 common + `own_existential_vtable_entries(principal)` + supertrait closure). The resulting slot count matches what rustc reports for `&dyn Trait` layout but can under-count Vacant slots vs. a specific `(principal, impl_ty)` pair.
- **Recommendation:** Either (a) document the deviation in the task spec — the spec text is literally infeasible at `Ty::Dynamic` translation time and the implementation choice mirrors rustc — or (b) defer vtable Struct layout to Phase 5 once the `Cast::Unsize` site reveals an `impl_ty`. The Phase-5 `intern_vtable_struct_ty` already calls `patch_layout` (`synth/vtable.rs:302-305`); the open question is whether the Phase-2 placeholder layout should be sized differently. In practice the placeholder is overwritten correctly, so this is mostly a doc/spec hygiene issue.

#### LOW-37-5 — `BridgeError::DynStar` vs spec's `BridgeError::UnsupportedDynKind`
- **File:** `bridge/mod.rs:184-187`, `bridge/ty.rs:155-159`.
- **Spec:** spec.md:5863 names the variant `UnsupportedDynKind`; task-spec line 186 names it `DynStar`.
- **Symptom:** Naming mismatch between the two specs. Implementation follows task-spec.
- **Recommendation:** Harmonise — pick one name and update the other doc.

#### LOW-37-6 — No `dyn*` integration fixture for DynStar rejection
- **File:** `crates/regseq-driver/tests/bridge_ty.rs` (DynStar is only a synthetic-enum-construction match at `bridge/ty.rs:884-889`).
- **Spec:** task-spec line 186.
- **Recommendation:** Add a `dyn*`-typed fixture if the nightly's `dyn_star` feature is enabled in the test crate; otherwise document why it's infeasible.

#### LOW-37-7 — Spec.md:6273 Test D ("for<'a> fn() -> &'a u32") not exercised as an integration fixture
- **File:** `crates/regseq-driver/tests/bridge_ty.rs`.
- **Spec:** spec.md §17.1 Test D (line 6273).
- **Recommendation:** Add a `type OutputHrtb = for<'a> fn() -> &'a u32;` + `pub fn out_hrtb(_: OutputHrtb) {}` integration fixture to match the §17.1 worked-example table.

---

## 2. regseq-driver-38 — Const Bridge and ConstMemoryStatic

**Verdict: PASS** — Per-variant §9.7 dispatch is complete, the audit-finding requirements (1) reference-typed const carrier as `Const::Aggregate([Reloc, Uint(0)])` and (2) no direct `model.items.push` are both implemented correctly, the Phase 2 → 4 → 5 pipeline is wired through `pending_items` and `HandleTable`, and integration tests cover every §9.7 row plus the audit-finding `&'static str` case.

### Findings

#### LOW-38-1 — Unit tests live in the integration file, not in-file
- **File:** `crates/regseq-driver/src/bridge/const_.rs` (no `#[cfg(test)] mod tests` block).
- **Spec:** task-spec lines 103-111 explicitly heading "Unit tests (`crates/regseq-driver/src/bridge/const_.rs` `#[cfg(test)] mod tests`)".
- **Symptom:** All per-scalar/ZST/cache-growth coverage lives in `crates/regseq-driver/tests/bridge_const.rs`. This is arguably more rigorous (real rustc fixtures vs. hand-rolled `mir::Const` mocks, which need a `TyCtxt` and are impractical), but it diverges from the task-spec wording.
- **Recommendation:** Either add a small in-file `mod tests` covering pure-helper paths that don't need a `TyCtxt` (e.g., `sign_extend` round-trips, the `translate_int_kind`/`translate_uint_kind`/`translate_float_kind` mappings), OR amend the task spec to acknowledge that hand-constructing `mir::Const<'tcx>` without rustc is infeasible. No spec-correctness consequence.

---

## 3. regseq-driver-39 — FnSig and C-ABI Bridge

**Verdict: PASS** — All §14.1/14.2/14.3/14.5 requirements implemented, including the four audit-finding refinements (position threading, cross-crate ADT resolution via `def_path_hash_to_def_id`, repr(C) ZST rejection, and `RustCall` rejection). 14 integration tests cover Direct + Substituted shapes, every audit-finding rejection row, and nested-FnPtr position inheritance.

### Findings

#### LOW-39-1 — By-value field cycle returns provisional `Direct`/`Ok` instead of `CyclicAggregateField` rejection
- **File:** `crates/regseq-driver/src/bridge/fnsig.rs:153-155` and `:216-218`.
- **Spec:** spec.md:5538-5542.
- **Symptom:** Re-entering an aggregate through a by-value field edge without a pointer/fnptr indirection should reject with `BridgeError::CyclicAggregateField`. The current implementation returns `Direct`/`Ok(())`. No `CyclicAggregateField` variant exists in `BridgeError`.
- **Mitigating factor:** Spec explicitly notes "this case is impossible for valid Rust sized types (rustc rejects such infinite-size types at definition)". The path is structurally unreachable from valid rustc input.
- **Recommendation:** Defer unless a validator finding demonstrates reachability; document the conscious deferral inline if kept.

#### LOW-39-2 — `compute_cabi_lowering` panics on inadmissible types for non-C ABI sigs
- **File:** `crates/regseq-driver/src/bridge/fnsig.rs:65-74`.
- **Spec:** task-spec lines 49-52 (signature returns `CAbiLowering`, not `Result`).
- **Symptom:** If invoked on a Rust-ABI FnSig with a `char` input (or other non-admissible-at-C type), the internal checker hits `.expect("...")`. Not reachable from current tests since the fixture set uses admissible types.
- **Recommendation:** Tighten to `if sig.abi != FnAbi::C { return Direct/Direct trivially; }`, OR keep `.expect` and document the precondition.

#### LOW-39-3 — Redundant `ensure_cabi_lowering_slots` calls
- **File:** `crates/regseq-driver/src/bridge/fnsig.rs:51-58`.
- **Symptom:** Slot-extension call happens twice per `translate_fn_sig` (once when constructing lowering, once when assigning). Functionally idempotent.
- **Recommendation:** No fix required.

---

## 4. regseq-driver-40 — Closure + VTable + Drop-Glue Item Synthesis

**Verdict: PASS** for the driver-40 deliverables themselves — `tcx.vtable_entries` consultation with full `VtblEntry` variant mapping, VTableInstance Static synthesis with `Const::Aggregate` init, dedup on PrincipalTraitKey/TyId, and Phase-5 rekey-determinism are all spec-conforming. **But two HIGH-severity cross-task coordination findings make the integrated pipeline non-conforming.**

### Findings

#### HIGH-40-1 — Closure captures-struct Item double-pushed by both `bridge/ty.rs` and `synth/closure.rs`
- **Files:**
  - `crates/regseq-driver/src/bridge/ty.rs:192-205, 355-358` — `intern_item(cx, *def_id, ItemKind::Struct, SpecialAdt::None)` pushes captures Item with `SyntheticItemKind::None` directly into `model.items`.
  - `crates/regseq-driver/src/bridge/synth/closure.rs:143-157` — `push_closure_captures_item` independently pushes a captures Item with `SyntheticItemKind::ClosureCaptures { closure }` through `pending_items` / HandleTable.
- **Spec:** task-spec lines 312-320 (audit-finding F4); spec.md §3.1 invariant 18 (uniqueness of `item_def_paths`).
- **Symptom:** Every closure expression produces TWO Items in `model.items` sharing the same `item_def_paths[id]` (the rustc closure DefPathHash). The integration test `run_bridge_synthesises_closure_vtable_and_drop_glue_fixtures` (`tests/bridge_synth.rs:361-416`) uses existence checks (`.any(...)`), so it tolerates duplicates and doesn't surface the bug. No validator runs after `run_bridge`, so the regression is silent.
- **Recommendation:** Route closure captures-Item registration through `synth/closure.rs` only. `bridge/ty.rs::translate_ty` for the Closure arm should look up the ItemHandle (or final ItemId post-Phase-4) from the synth cache and use it for both the `Ty::Closure(item, args)` interning and the `Ty::Struct { item, .. }` view, instead of calling `intern_item`. This is the same refactor required to fix HIGH-37-1.

#### HIGH-40-2 — VTableInstance Statics pushed during Phase 5 before Function Instances; future driver-42 will produce F4-2 violation
- **Files:** `crates/regseq-driver/src/bridge/synth/mod.rs:217`, `crates/regseq-driver/src/bridge/synth/vtable.rs:272`.
- **Spec:** task-spec lines 305-310 (audit-finding F3); spec.md §16.2 F4-2 (Function rank 0, Static rank 1).
- **Symptom:** `push_vtable_instance_statics` pushes Static Instances to `model.instances` during Phase 5. The doc-comment at `synth/mod.rs:140-143` explicitly notes the design depends on driver-42's `construct_function_instances` to land later. But once driver-42 lands and pushes Function instances *after* the existing Statics, the final `model.instances` will be `[Static, Static, ..., Function, Function]`, violating F4-2 (Function rank 0 must precede Static rank 1). The driver-40 test `vtable_items_and_instances_are_f4_and_f4_2_ordered_post_phase4` (`tests/bridge_synth.rs:616-664`) passes only trivially because no Function instances exist in the current driver-40-only build.
- **Recommendation:** Defer `push_vtable_instance_statics` into a Phase-5 sub-pass that runs AFTER driver-42's Function instance construction, OR hold the Static initialisers in a side-table and final-sort `model.instances` by F4-2 immediately before workspace-validator entry. The fix should land in coordination with driver-42's re-execution.

#### MEDIUM-40-3 — Closure captures-struct field names are `""` instead of `_0, _1, ...`
- **File:** `crates/regseq-driver/src/bridge/ty.rs:673-687` (`intern_closure_captures_struct_view`).
- **Spec:** spec.md §13.1 lines 4527-4529 ("Fields = the captured upvars (per rustc's `tupled_upvars_ty()`), flattened to named fields (`_0`, `_1`, ...)").
- **Symptom:** All captures-struct field names interned as the empty string. This is technically driver-37's territory (it owns the parallel `Ty::Struct` view), but it's a closure-family cross-concern.
- **Recommendation:** Intern field names as `_{idx}` (formatted decimal) for each upvar position.

#### LOW-40-4 — Stale doc-comment on `rekey_synthesis_in_phase5`
- **File:** `crates/regseq-driver/src/bridge/synth/mod.rs:82-85`.
- **Symptom:** Doc says `vtables[principal] = vtable_instance_item_id`. The audit-finding-corrected implementation maps to `vtable_TYPE_item_id` (correctly), per spec.md:4797-4805. The implementation is right; the comment is stale.
- **Recommendation:** One-line doc-comment update.

#### LOW-40-5 — VTable DefPath rendering uses §13.4 canonical segments rather than §13.2 display form
- **File:** `crates/regseq-driver/src/bridge/synth/vtable.rs:574-609`.
- **Spec:** spec.md:4601-4602 (display-only `<vtable_ty for Trait[..][..]>`) vs spec.md §13.4 (canonical segments).
- **Symptom:** Rendered form is the canonical `<regseq_synth>::vtable_ty::<hash>::<args>::<bindings>` chain. The spec carves this as a display-only convention; either form is "correct" but the §13.2 advertised form is not produced.
- **Recommendation:** Add an explicit code-comment noting the §13.4 segment-based rendering is authoritative and the §13.2 display form is purely a convention.

---

## 5. Cross-cutting findings

### XC-1 — One root cause produces two HIGH findings
Both HIGH-37-1 (intern_item bypasses pending_items) and HIGH-40-1 (closure captures Item double-pushed) share the same root: `bridge/ty.rs::intern_item` writes directly to `cx.model.items` instead of routing through `pending_items` + HandleTable. Fix once; both findings clear.

### XC-2 — F4-2 ordering is brittle and will break on driver-42 re-execution
HIGH-40-2 is structural: the current driver-40 implementation relies on a not-yet-implemented driver-42 pushing Function Instances correctly. Until driver-42 lands and a workspace validator runs end-to-end, no test in the suite can detect the violation. **Recommendation:** add a Phase-5 post-pass that explicitly `model.instances.sort_by_key(|inst| (f4_2_rank(inst), inst.def_path_hash))` rather than depending on push order.

### XC-3 — String-matching as a stand-in for structural Ty visitors
MEDIUM-37-3 (substantive-HRTB predicate) is the only place I found this pattern, but it's worth flagging as a class: any predicate that walks rendered Ty text rather than the Ty graph is technical debt against rustc API changes and pretty-printer drift.

### XC-4 — Test surface gaps
The integration tests are otherwise thorough, but two gaps surfaced:
- LOW-37-7 (output-position HRTB test) and LOW-37-6 (DynStar source fixture) — driver-37.
- LOW-40-4's behaviour (vtables map to VTableType ItemId) is tested correctly, but no validator currently asserts `item_def_paths` uniqueness post-rekey, masking HIGH-40-1.
- Per HIGH-37-2, no terminator-level integration test exercises a direct `Fn::call`/`FnMut::call_mut`/`FnOnce::call_once` against a closure.

### XC-5 — `compute_cabi_lowering` panic surface (LOW-39-2)
Minor, but worth tightening before driver-42 starts calling this from terminator translation paths.

---

## 6. Build & quality gate (workspace-wide spot-check)

All four task spec Quality Gates passed individually in each agent run:
- `cargo test -p regseq-driver --no-run` — green
- `cargo test -p regseq-driver --lib bridge::{ty,const_,fnsig,synth}` — green
- `cargo test -p regseq-driver --test bridge_{ty,const,fnsig,synth}` — green
- `cargo clippy -p regseq-driver -- -D warnings` — green
- `cargo fmt --all -- --check` — green

Caveat: tests pass with the HIGH-37-1 / HIGH-40-1 duplicate-Item defect present because no test asserts `item_def_paths` post-rekey uniqueness, and tests pass with HIGH-37-2 unimplemented because no terminator-level closure-call test was added.

---

## 7. Recommended next steps (in priority order)

1. **Fix HIGH-37-1 / HIGH-40-1 together.** Refactor `bridge/ty.rs::intern_item` to use the HandleTable / pending_items pattern. Remove the duplicate captures-struct path in `bridge/ty.rs`'s Closure arm. Add a validator assertion that `item_def_paths` is unique post-rekey.
2. **Fix HIGH-37-2.** Wire the closure-call shim rewrite at `bridge/body/terminator.rs::translate_call_callee`. Add a terminator-level integration test exercising a direct closure call.
3. **Decide on HIGH-40-2 strategy.** Either (a) defer vtable Static push to a Phase-5 sub-pass that runs after driver-42, or (b) add an F4-2 sort post-pass on `model.instances`. Land coordinated with driver-42 re-execution.
4. **Replace string-matching HRTB predicate (MEDIUM-37-3)** with a structural `TypeVisitor`.
5. **Resolve MEDIUM-37-4 (vtable slot count).** Either document the spec deviation explicitly or move slot-count derivation to Phase 5 when `impl_ty` is known.
6. **Fix MEDIUM-40-3** (`_0, _1, ...` field naming).
7. **Address LOW findings opportunistically** — they're polish, doc-hygiene, and test-surface gaps.

---

## Appendix: artefacts surveyed

- Task specs: `.harness/tasks/regseq-driver/{37,38,39,40}-*.md` (v4/v2 each).
- Spec sections: spec.md §3, §3.4, §9.7, §13.1, §13.2, §13.3, §13.4, §14.1, §14.2, §14.3, §14.4, §14.5, §15.0, §15.1, §16.2, §17.1, Appendix A.1.
- Implementation: `crates/regseq-driver/src/bridge/{mod,ty,const_,fnsig,handles,assign}.rs`, `crates/regseq-driver/src/bridge/synth/{mod,closure,vtable,drop_glue}.rs`, `crates/regseq-driver/src/bridge/body/terminator.rs`.
- Tests: `crates/regseq-driver/tests/bridge_{ty,const,fnsig,synth}.rs` plus in-file `#[cfg(test)]` modules.
- regseq-model surface: `crates/regseq-model/src/{model.rs, item.rs, synthetic.rs, fn_sig.rs, ty_interner.rs, const_value.rs}`.
- Pinned rustc source: `~/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/ty/{vtable.rs, erase_regions.rs, sty.rs, closure.rs}`.
