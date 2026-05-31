# Phase 1 — vtable / dyn-trait-object / closure-fn-pointer lowering plan

A **plan-first** doc (pre-spec, per `.harness/generic-c/methodology.md`) for the
"**vtable task**": the last in-scope Phase-1 backend gap. It greens the three
codegen-coverage rows that fail loud in the backend today —
**`TY-14`** (`Ty::Dynamic` / dyn trait-object fat pointer), **`PC-3`**
(`PointerCoercion::ClosureFnPointer`), **`IN-6`** (`virtual_idx`, the vtable
slot index, `Some` only for `Virtual` instances). These are the
"vtable-task-blocked" rows of the §06 fixture-coverage plan (§4b).

This doc **resolves the forks** the matrix routed "→ Doc 2" (the IN-6 note:
"§9.6 *illustrates* one vtable instance; the general slot-emission rule is not
pinned"), HEAD-grounded, so the follow-on spec delta + implementation are
mechanical. **Per the architect decision (path = "plan-first design doc, then
stop for review"), nothing is implemented until this doc is reviewed.**

---

## 0. The three rows (the gate target)

| row | construct | matrix anchor |
|----|-----------|---------------|
| `TY-14` | `Dynamic { auto_traits, vtable_ty }` — fat pointer `{ void* data; const VTable* vtable }` behind `Ref`/`Ptr` | gc §3.2, §3.3, §9.6 |
| `PC-3` | `PointerCoercion::ClosureFnPointer` — non-capturing closure → fn ptr | spec §21.3.3 |
| `IN-6` | `FunctionInstanceData.virtual_idx: Option<u32>` — vtable slot index for `Virtual` instances | gc §9.6 |

A row greens on **one** committed fixture that compiles + runs + matches. So the
deliverable is: (a) make the backend emit the dyn/vtable/closure constructs, and
(b) two fixtures — one dyn-dispatch (`TY-14` + `IN-6`), one closure→fn-ptr
(`PC-3`). Ledger effect: the dyn-dispatch rows take `covered-without-green`
**7 → 5**, and `PC-3` takes it **5 → 4** — landing separately if `PC-3` splits
into its own task (§D7 / §6). The end-state residual is **4**: `IT-14` opt-in +
the `DL-5`/`CO-12`/`CK-11` dispositions.

---

## 1. Current state (HEAD-grounded)

### 1.1 The model + bridge are COMPLETE — the gap is backend emission only

The bridge already synthesizes everything the backend needs:

- **`Ty::Dynamic { auto_traits: Vec<AutoTrait>, vtable_ty: TyId }`**
  (`regseq-model/src/ty.rs:263`). `vtable_ty` is a synthesized **`Ty::Struct`**.
- **Vtable struct layout** —
  `regseq-driver/src/bridge/synth/vtable.rs:589` `vtable_layout_for_slots`:
  one pointer-sized field per rustc `vtable_entries` slot, **in slot order**:
  `__drop_in_place` (`Ptr(unit, Mut)`), `__size` (`Usize`), `__align` (`Usize`),
  then each method (`FnPtr(shim_fn_sig)` — receiver erased to `*mut ()`, C ABI),
  then supertrait-vptr / vacant slots. Field name = the method name (collision
  suffix `#<idx>` on dup).
- **Vtable instance** — `vtable.rs:460` `build_vtable_init_aggregate`: a
  `Const::Aggregate` of `Reloc`(drop/method shims) + `Uint`(size/align) +
  null/uninit, carried by a `SyntheticItemKind::VTableInstance { principal,
  self_ty }` **static** item.
- **Virtual-call rewrite** — `regseq-driver/src/bridge/body/vtable_indirect.rs`
  already rewrites a virtual call into explicit MIR (5 synthetic locals):
  `recv = receiver`; (shared-borrow only) `raw = &(*recv)`;
  `meta = PtrMetadata(recv)`; `func = (*meta).field[slot]`;
  `data = Cast(PtrToPtr, raw_or_recv, *mut ())`; then
  `callee = Indirect(Move(func))`, `args[0] = Move(data)`. **The backend never
  indexes the vtable itself — it only emits these projections.**
- **`FunctionInstanceFlavor::Virtual` + `virtual_idx: Option<u32>`**
  (`instance.rs:28`, `:203`); `SyntheticItemKind::{VTableType, VTableInstance,
  VTableMethodShim { vtable_entry_index }, VTableDropShim}` (`synthetic.rs:87`);
  `RegseqModel.vtables: HashMap<PrincipalTraitKey, ItemId>` (`model.rs:104`).
- **`PointerCoercion::ClosureFnPointer`** (`rvalue.rs:104`) is translated 1:1
  by the bridge (`bridge/body/rvalue.rs` `translate_cast_kind`); the closure's
  call method is a `SyntheticItemKind::ClosureCall { closure, call_kind }`.
- A passing end-to-end bridge test exists:
  `regseq-driver/tests/bridge_vtable_indirect_end_to_end.rs` (model validates;
  the rewrite is present). The `trait_object.rs` fixture
  (`test-ir-fixtures/fixtures/trait_object.rs`) is the canonical dyn source.

### 1.2 What the backend ALREADY emits

- Fat-pointer **forward decls** + **struct definition**: `mod.rs:689` (forward),
  `mod.rs:736` → `types::emit_trait_obj_struct` (`types.rs:335`) emits
  `struct trait_obj_<Trait> { void *data; const struct <Trait>_VTable *vtable; }`.
- `CallCallee::Indirect(op)` → `({op})(args)` (`body_emit.rs:1970`) — the
  indirect call already works (b5 `TM-9`).
- Thin-pointer `Deref` + struct `Field` (`place_walk`, `body_emit.rs:1062`) —
  so `(*meta).field[slot]` (meta is a **thin** `Ptr(vtable_struct)`) needs **no
  dyn-specific code**; it is a normal struct deref+field.

### 1.3 The backend work surface — six loud `dyn`/closure sites, one non-loud trap, one out-of-scope loud site

Loud `EmitError::Unsupported` sites (`body_emit.rs`):

| # | site | message | hit by |
|---|---|---|---|
| 1 | `place_walk` `Deref`, ~`:1111` | "Deref of a dyn pointer (vtable task)" | `&(*recv)`, any `*dyn` |
| 2 | `fat_reborrow`, ~`:1384` | "Ref of a dyn deref (vtable task)" | shared-borrow `raw = &(*recv)` |
| 3 | `raw_ptr_aggregate`, ~`:1554` | "RawPtr aggregate to a dyn pointer (vtable task)" | a `*dyn` (raw, not ref) build |
| 4 | `pre_rvalue` PtrMetadata, ~`:1641` | "PtrMetadata of a dyn pointer (vtable task)" | `meta = PtrMetadata(recv)` |
| 5 | `unsize_cast`, ~`:1823` | "Unsize to a dyn pointer (vtable task)" | `&T as &dyn Tr` |
| 6 | `cast_expr`, ~`:1758` | "PointerCoercion::ClosureFnPointer (vtable task)" | closure → fn ptr (`PC-3`) |

Plus two NON-table items the Codex round-1 review surfaced:

- **Non-loud trap (7th work item):** `PtrToPtr` emits `(({dest})({e}))`
  (`body_emit.rs:1750`). For a **fat dyn source** cast to a thin `void*` (the
  bridge's `data = Cast(PtrToPtr, raw_or_recv, *mut ())`), that casts the whole
  struct — wrong; it must extract `.data` first: `((void*)((e).data))`. This
  fails to compile (not a loud `EmitError`), so it is easy to miss.
- **Out-of-fixture-scope loud site (8th):** `body_emit.rs:3308` —
  "Const::Aggregate reference carrier to a dyn pointee: vtable task" — fires only
  for a **`const`/`static` holding a `&dyn`**. The gate fixtures use a *runtime*
  `&dyn` (the unsize happens in a function body), so this is **not on the gate's
  critical path**; it may stay a documented limitation (or be a stretch goal).

`mod.rs:1461` `record_ref_storage` silently `Ok(())`s a `Dynamic` pointee — this
is in the **const-memory-static storage prepass** (`mod.rs:1003`), not a runtime
blocker; revisit only if the const-`&dyn` site (8th) is taken.

### 1.4 What the spec PINS vs. leaves open

**Pinned (gc = `generic-c-backend.md`):** §3.2 (dyn fat-pointer shape), §3.3
(`data` slot unqualified `void*` erasure, rationale in model `spec.md`
§13.2/§17.2.3), §9.6 (full worked-example C: vtable struct type, the
`const VTable __vtable_X_for_Y = {...}` initializer, the `d.vtable->read(d.data)`
virtual call), §10 (`Unsize`→`{data, vtable=&VT}`; `ClosureFnPointer`→`&<call
shim>`), §7 (`virtual_idx: Some(i)` → vtable slot `i` = `&<shim>`).

**Open ("→ Doc 2") — what THIS doc resolves:**
1. The **general** vtable-struct field order/naming + the instance initializer
   rule (§9.6 is one example only).
2. **Where the `Unsize`-to-dyn vtable reference comes from** (rustc MIR carries
   no vtable operand at the cast — it is materialized in codegen).
3. `ClosureFnPointer` — *which* shim, and how it is reached/emitted.
4. `PtrMetadata` of a dyn ref → `.vtable` (only `.len` for slice is pinned/impl).
5. A §9.6 spec-vs-bridge **field-name mismatch**: §9.6 prints `drop_in_place` /
   `size` / `align` / `read`; the bridge layout uses `__drop_in_place` /
   `__size` / `__align` / `<method>`.

---

## 2. Decisions to settle (the forks)

### D1 — Vtable struct TYPE emission: reuse the normal struct path

`vtable_ty` is a `Ty::Struct` with fully-typed fields (§1.1). **Decision:** emit
it via the **existing struct-definition path** (banks 2-3), not a special case —
the fields are ordinary `Ptr`/`Usize`/`FnPtr` C types the backend already emits.

*Reachability (corrected per Codex round-1, issue 2):* the vtable struct type,
the `VTableInstance` static, and the method/drop shim instances are **created by
the bridge** during `synthesise_vtable_with_rustc_entries`
(`bridge/synth/vtable.rs:302-347`) — they already exist in the model's item /
instance set. The backend's type/instance **discovery roots on instance
signatures, locals, and static/const types** (`mod.rs:367-395`), so these
emit **because they are in the bridge-built instance set**, *not* because a
`Reloc` references them (a `Reloc` only references + validates target existence,
`model.rs:3312`). **Verify empirically** that the discovery walk reaches the
synthetic vtable-type/instance/shims; add an explicit emission root only if a
fixture shows a gap.

*Field-name reconciliation:* the emitted C carries the **bridge's** names
(`__drop_in_place`, `__size`, `__align`, `<method>`; `vtable.rs:589`). §9.6's
example (`drop_in_place`/`size`/`align`/`read`) is illustrative — the **spec
delta reconciles §9.6** to the bridge-derived names. No code chooses these; the
struct path emits the model's field names verbatim.

### D2 — Vtable INSTANCE emission: a normal static + `Const::Aggregate`

The `VTableInstance` item is a **static** whose init is a `Const::Aggregate`
(`vtable.rs:460`). **Decision:** emit it via the **existing static +
`Const::Aggregate` path** (§12). Slot values are `Reloc`(shim) / `Uint`
(size/align) / null / uninit — all already lowerable (the fn-pointer fields take
`&<shim emission_name>`, a `Reloc` → `&<emission_name>` per §12). No new emission
machinery; reachability is per D1 (bridge-created instances + discovery roots).

*Vacant-slot reconciliation (Codex round-1, issue 5):* `build_vtable_init_aggregate`
emits an **uninit** cell for a `Vacant` slot (`vtable.rs:513`), while
`spec.md:4681` describes a null. The spec delta must pin which (recommend
matching the bridge: uninit), so the `Const::Aggregate` → struct initializer is
unambiguous.

### D3 — Virtual-call lowering: emit the bridge's projections (no vtable indexing)

The bridge already lowered the call (§1.1). **Decision:** the backend just makes
the projections in that rewritten MIR emit:
- `meta = PtrMetadata(recv)` → **D5** (`.vtable` of the dyn fat pointer).
- `func = (*meta).field[slot]` → already works (thin-ptr deref + struct field).
- `raw = &(*recv)` / `data = Cast(PtrToPtr, ., *mut ())` → **D6** (dyn place
  mechanics + the fat→thin `PtrToPtr` `.data` extraction).
- `(func)(data, args)` → already works (`CallCallee::Indirect`).

So `IN-6` greens with **no vtable-indexing logic in the backend** — only the dyn
fat-pointer field reads (D5/D6).

### D4 — `Unsize`-to-dyn vtable source: bridge resolves + records, backend builds the (type-correct) Ref fat struct

The crux. `&T as &dyn Tr` is one `Cast(PointerCoercion(Unsize), src, &dyn)`; the
vtable is **not** a MIR operand (rustc materializes it in codegen). The
**destination is a `Ty::Ref`** (a *reference* fat pointer), which kills the
round-1 idea of a `RawPtr` aggregate: `AggregateKind::RawPtr` recovers
`Ty::Ptr` (a *raw* pointer), not `Ty::Ref` (`body_ty.rs:314`; `spec.md:7606`) —
**type-incorrect for `&dyn`** (Codex round-1, issue 1). The corrected options:

- **Option A — backend resolves end-to-end.** `unsize_cast` for a `Dynamic`
  pointee recovers the principal from the dyn, computes `src`'s pointee
  `self_ty`, looks up `VTableInstance { principal, self_ty }`, and builds the
  **Ref fat struct literal** `(struct trait_obj_T){ .data = (void*)(src),
  .vtable = &<instance> }`. **Cons:** duplicates the resolution the bridge
  already performs at unsize sites (below); needs backend `self_ty`/`ArgsKey`
  derivation + synthetic-item indexing.

- **Option B (recommended) — bridge resolves + records, backend builds.** The
  bridge **already visits every unsize site** to synthesise the vtable
  (`mod.rs:1325-1349`: it matches `Cast(Unsize, operand, target_ty)`, derives the
  `principal` from `dynamic_principal_ref(target_ty)` + the `self_ty` from
  `concrete_impl_ty(operand.ty())`, and calls `synthesise_vtable_with_rustc_entries`).
  **Extend that pass to also record the resolved `VTableInstance` ItemId** in a
  small model side-table (e.g. keyed by `(dyn TyId, self_ty ArgsKey)`, or by the
  unsize site). The backend's `unsize_cast` (site #5) then reads the table and
  builds the **type-correct Ref fat struct literal** `(struct trait_obj_T){ .data
  = (void*)(src), .vtable = &<instance> }` — matching the `Ty::Ref(Dynamic)`
  destination, no IR-node abuse and no `RawPtr`/`unsize_cast` mismatch. Site #3
  (`raw_ptr_aggregate` for dyn) is then only for a genuine **`*dyn` raw** pointer
  build, kept symmetric with the slice path.

**Recommendation: Option B.** It reuses the bridge's existing unsize-site
resolution (so it does not duplicate logic), keeps the cast **type-correct** (the
backend builds a `Ref` struct literal, not a `RawPtr` aggregate), and confines
the model change to a small side-table (no new rvalue/IR variant). **Architect
decision needed:** B touches the bridge (extend the unsize-scan pass + add the
side-table + a test); A keeps it backend-pure but re-implements the
principal/`self_ty`→instance resolution in the backend. Either way the backend
**builds the struct literal** (the type-correct part); the only question is
*where the `VTableInstance` ItemId is resolved*.

### D5 — `PtrMetadata` of a dyn ref → `.vtable`

**Decision:** extend `body_emit.rs:1631` `UnOp::PtrMetadata`: a `Dynamic`
pointee yields `({e}).vtable` (the §3.2 second slot), exactly mirroring the
existing `Slice/Str → ({e}).len`. One arm.

### D6 — dyn fat-pointer place mechanics + the fat→thin `PtrToPtr` (the shared helpers)

**Decision:** mirror the slice `{data,len}` machinery for dyn `{data,vtable}`:
- `place_walk` `Deref` (#1): a `Dynamic` pointee sets `lv = "{lv}.data"` and
  remembers the fat base (so a later `PtrMetadata`/reborrow reaches `.vtable`),
  mirroring the `Slice/Str` arm.
- `fat_reborrow` (#2): return the fat base (identity copy of `{data, vtable}`),
  mirroring the slice reborrow.
- **Fat→thin `PtrToPtr` (#7, the non-loud trap):** `body_emit.rs:1750` currently
  emits `(({dest})({e}))`, which casts the *whole fat struct* — a C error for a
  dyn source. **Add** a dyn-fat-source case that extracts `.data` first:
  `(({dest})((e).data))`. Required by the bridge's `data = Cast(PtrToPtr,
  raw_or_recv, *mut ())`. This is the one item that fails to *compile* rather
  than fail loud, so it must be explicit in the implementation plan.

### D7 — `ClosureFnPointer` → a concrete no-receiver shim (`PC-3`)

`PC-3` is the **least settled** row (Codex round-1, issue 3). `ClosureFnPointer`
is translated 1:1 by the bridge (`bridge/body/rvalue.rs`), and closure synthesis
creates **multiple** `ClosureCall` arms with **different receiver shapes**
(`bridge/synth/closure.rs:48-58,:129-140`; receiver handling `mod.rs:2631-2641`).
A `fn(args)->ret` pointer has **no receiver**, but a `ClosureCall` shim takes the
(ZST, for non-capturing) closure env as its receiver. **Decision (to confirm):**
the `ClosureFnPointer` target is the **`CallOnce` arm with ZST-receiver elision**
(the empty non-capturing env is a ZST, dropped at the C boundary), emitted as
`&<emission_name>`; the spec delta must pin this and **prove the emitted C
signature matches `Ty::FnPtr(sig)`** (no receiver param). **Open risk:** if no
existing shim has the no-receiver C signature, `PC-3` needs a small **bridge
synth** (a fn-ptr trampoline) — which would make `PC-3` a heavier sub-task than
`TY-14`/`IN-6`. **Recommend a short spike** to confirm the shim source before
committing `PC-3`'s scope; if it balloons, `PC-3` can split into its own task
(it is independent of the dyn-dispatch rows). Closures are otherwise covered by
`b9_closures` (closure *calls*, not the fn-pointer coercion).

---

## 3. Spec-delta assessment (authored AFTER this doc is approved)

The target C is pinned (§3.2/§3.3/§9.6/§10/§7). The delta needed is the
**general rule**, mostly a generalization + reconciliation, not new C shapes
(scope expanded per Codex round-1, fix 5):

1. **§9.6 → general vtable rule.** Promote the worked example to the general
   rule: vtable struct field order = rustc `vtable_entries` order
   (`__drop_in_place`, `__size`, `__align`, methods by slot, supertrait-vptr,
   vacant); the `VTableInstance` static initializer fills slot `i` =
   `&<shim>` / `sizeof`/`_Alignof` / **uninit** (vacant). Reconcile the §9.6
   field names (`__`-prefixed) **and** the **vacant-slot null-vs-uninit** mismatch
   (`spec.md:4681` null vs `vtable.rs:513` uninit; recommend uninit). — **the
   IN-6 "→ Doc 2" item.**
2. **`Unsize`-to-dyn rule** (per D4, corrected): pin the **Ref fat-struct
   construction** `{ .data = (void*)src, .vtable = &<VTableInstance> }` (a
   `Ty::Ref` destination — **not** a `RawPtr` aggregate, which is `Ty::Ptr`,
   `body_ty.rs:314`). If Option B, also spec the bridge side-table that records
   the resolved `VTableInstance` ItemId at the unsize site (model-side,
   `spec.md` §25.7-family); if Option A, spec the backend's resolution rule.
3. **`ClosureFnPointer`** (§10): pin the concrete shim — the **`CallOnce` arm
   with ZST-receiver elision** (or a dedicated no-receiver trampoline) — and that
   its emitted C signature matches `Ty::FnPtr(sig)` (no receiver param). Tightens
   the current vague "synthesised call shim".
4. **`PtrMetadata` of dyn → `.vtable`** (the `UN-3` metadata read; §10/§3.2).
5. **Dyn fat→thin `PtrToPtr` `.data` extraction** (§3 / §10 cast rules): a
   `PtrToPtr` from a `Ref/Ptr(Dynamic)` to a thin pointer reads `.data`, not the
   whole struct.

**The doc's stance: a spec delta is warranted** (the matrix flags the general
rule unpinned), scoped to items 1–5. The exact IR shape for D4 (side-table vs.
backend resolution) is the open architect choice that fixes item 2's wording.

---

## 4. Implementation plan (after spec delta; ordered, each independently verifiable)

*(Option B path; if Option A is chosen, step 1 moves into the backend and adds
explicit discovery roots.)*

1. **(Option B) bridge: record the unsize-site `VTableInstance`.** Extend the
   existing unsize-scan (`mod.rs:1325-1349`, which already derives principal +
   `self_ty` and synthesises the vtable) to resolve + record the
   `VTableInstance` ItemId in a model side-table; unit + end-to-end test (extend
   `bridge_vtable_indirect_end_to_end.rs`). → verify: the table maps each unsize
   site to its instance; model validates.
2. **Backend site #5** (`unsize_cast` for dyn): build the **Ref fat-struct
   literal** `(struct trait_obj_T){ .data = (void*)(src), .vtable =
   &<instance> }` from the side-table. → verify: emits the §3.2 shape, type
   matches the `Ty::Ref(Dynamic)` destination.
3. **Backend sites #1/#2/#4 + #7** (dyn `Deref` `.data`, dyn reborrow,
   `PtrMetadata` `.vtable`, fat→thin `PtrToPtr` `.data`). → verify: the
   bridge's virtual-call projections emit + compile.
4. **Backend site #6** (`ClosureFnPointer` → `&<no-receiver shim>`) — gated on
   the D7 spike. → verify.
5. **Discovery/emission**: confirm the bridge-created vtable struct type +
   `VTableInstance` static + shims emit (D1/D2); add explicit roots only if a
   fixture shows discovery misses them.
6. After each: `cargo run -p codegen-coverage-runner` + the two new fixtures.

## 5. Test plan (the fixtures that green the rows)

- **`b_vtable` (or `b6`-adjacent), `expect = "pass"`** — a `dyn` trait + a
  concrete impl + a `drive(&dyn Tr)` call site (the `trait_object.rs` shape,
  re-authored as a codegen-coverage fixture driving an observable value through
  the virtual call and logging the result). Covers **`TY-14` + `IN-6`** (the
  `Ty::Dynamic` fat pointer + the `Virtual`/`virtual_idx` vtable dispatch). The
  golden.c proves the vtable struct + instance + `d.vtable->m(d.data)` shape.
- **`b_closure_fnptr`, `expect = "pass"`** — a **non-capturing** closure coerced
  to a `fn(...)` pointer, then called indirectly; log the result. Covers
  **`PC-3`**. (Distinct from `b9_closures`, which exercises closure *calls*, not
  the fn-pointer coercion.)

Both follow the `codegen-fixture-no-core-methods` recipe (drive values through
operators/constructs; observe via the diag-log `info!`).

## 6. Residual & risks

- **D4 is the real architect decision** (bridge-records-side-table vs.
  backend-resolves); both build the **type-correct Ref fat struct** in the
  backend — the round-1 `RawPtr`-aggregate idea was type-incorrect (`Ty::Ptr` ≠
  `Ty::Ref`) and is dropped. Recommend Option B; needs sign-off because it
  touches the bridge.
- **`PC-3` is the least-settled row** and may be heavier than `TY-14`/`IN-6`: the
  `ClosureFnPointer` target must be a **no-receiver** shim whose emitted C
  signature matches `Ty::FnPtr` (D7). If no existing shim fits, `PC-3` needs a
  small bridge synth — recommend a **short spike first**, and split `PC-3` into
  its own task if it balloons (it is independent of the dyn-dispatch rows, so
  `TY-14`/`IN-6` can land without it; the ledger would go 7 → 5, then → 4 when
  `PC-3` lands).
- **Fat→thin `PtrToPtr` (#7) fails to *compile*, not loud** — easy to miss; it is
  an explicit implementation step, not a fail-loud site.
- **Const/static `&dyn` (8th site, `body_emit.rs:3308`) is out of gate scope** —
  the fixtures use a runtime `&dyn`; this stays a documented limitation unless
  taken as a stretch.
- **Reachability is bridge-created, not reloc-driven** — D1/D2 rely on the
  discovery walk reaching the bridge-synthesised vtable type/instance/shims;
  verify empirically (a missed root is a fixable backend root, not a redesign).
- **Soundness**: dyn `data`-slot `const` erasure (§3.3) must be preserved — the
  emitted `data` is unqualified `void*` regardless of borrow flavor.

## 7. Grounding anchors

- Backend sites: loud — `regseq-backends/src/generic_c/body_emit.rs`
  `:1111` (Deref), `:1384` (reborrow), `:1554` (RawPtr agg), `:1641`
  (PtrMetadata), `:1823` (Unsize), `:1758` (ClosureFnPointer), `:3308` (const
  `&dyn` carrier, out of scope); non-loud trap — `:1750` (`PtrToPtr` casts the
  whole expr). Discovery roots: `mod.rs:367-395`. Forward decls / fat struct:
  `mod.rs:689`/`:736`, `types.rs:335`. Const-storage prepass dyn stub:
  `mod.rs:1003`/`:1461`.
- Model: `ty.rs:263` (`Dynamic`), `instance.rs:28`/`:203`
  (`virtual_idx`/`Virtual`), `rvalue.rs:104` (`PointerCoercion`),
  `synthetic.rs:87` (synthetic kinds), `model.rs:104` (`vtables`); `RawPtr` →
  `Ty::Ptr` (`body_ty.rs:314`); `Reloc` validates target (`model.rs:3312`).
- Bridge: `bridge/synth/vtable.rs:302-347` (creates the instances/shims),
  `:460` (init aggregate; vacant=uninit `:513`), `:589` (layout),
  `bridge/mod.rs:1325-1349` (unsize-site vtable synthesis = the D4-Option-B
  hook), `bridge/body/vtable_indirect.rs` (call rewrite), `bridge/body/rvalue.rs`
  (`Unsize` translated 1:1), `bridge/synth/closure.rs:48-58`/`:129-140` +
  `bridge/mod.rs:2631-2641` (closure shim arms / receivers).
  Test: `tests/bridge_vtable_indirect_end_to_end.rs`.
- Spec: gc §3.2/§3.3 (`generic-c-backend.md:116`/`:147`), §9.6 (`:646`),
  §10 PointerCoercion (`:917`), §7 shim flavors; model `spec.md` §13.2/§17.2.3
  (receiver erasure), §21.3.3 (`PC-3`).
- Matrix rows: `phase-1/01-construct-surface-and-matrix.md` `TY-14`/`PC-3`/`IN-6`.
- Fixture recipe + ledger: §06 plan §4b; `codegen-fixture-no-core-methods`.
