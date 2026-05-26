# Plan — RJ-013-fix: PostAnalysis reveal of RPIT opaque types at the bridge type entry

Plan-first design doc (pre-spec). Grounded at HEAD (`5764238`). The second
(and last) Bucket-B real fix from the driver-80 triage
(`.harness/audits/2026-05-25-needs-validation-tail-triage.md`); the A1-residue
sibling of the fixed RV-007-C / FF-005 projection-normalization cluster.
Probe-first per the R-series discipline.

## 0. Goal

A body that **uses a value whose type is a return-position `impl Trait` (RPIT)
opaque from a callee** fails to bridge:

```rust
#![no_std]
pub fn mk() -> impl Fn() -> u32 { || 0 }
pub fn f() { let _g = mk(); }
```

→ `BridgeOutcome::Rejected(NonRigidTy { kind: Alias, ty_repr: "impl Fn() -> u32" })`
(reproduced at HEAD). Goal: such a body bridges `Ok` + validates (the opaque is
revealed to its concrete hidden type), with no regression to any existing row.

## 1. HEAD-grounded probe results (this session, throwaway `try_bridge_fixture_path`)

Two probe rounds (7 + 5 witnesses) through the real bridge at `5764238`. The
probe **overturned both prior characterizations** of the projection witnesses
and sharpened the gap to RPIT opaques only:

| Witness | Outcome at HEAD |
| ------- | --------------- |
| P1 `let _g = mk()` (RPIT `impl Fn`) | **Rejected(Alias, "impl Fn() -> u32")** ← the gap |
| P2 `mk()` defined, never used | **Ok** (opaque bridges in its *defining* fn) |
| P3 `let g = mk(); g()` | **Rejected(Alias)** |
| P4 `impl Copy` RPIT used | **Rejected(Alias, "impl core::marker::Copy")** ← NOT Fn-specific |
| P5 `<u32 as Tr>::A` as return type | **Ok** (rustc pre-normalizes the concrete projection) |
| P6 `PhantomData<<u32 as Tr>::A>` | **Ok** (NOT `Param`, NOT `Alias` — both prior claims stale) |
| P7 `impl FnMut` RPIT used | **Rejected(Alias)** |
| Q1 bare closure local `let _g = \|\| 0u32` | **Ok** (closure-typed local bridges) |
| Q2 bare closure called | **Ok** |
| Q3 `fn f(_g: impl Fn() -> u32)` (APIT arg) | **Ok** (generic → monomorphized) |
| Q4 `sink(mk())` (opaque temp into call) | **Rejected(Alias)** |
| Q5 nested RPIT (`outer()` returns `inner()`'s opaque) | **Rejected(Alias)** |

**Conclusions:**
1. The genuine RJ-013 gap is exactly: an **`AliasTyKind::Opaque` (RPIT)** type
   reaching `translate_erased_ty` in a **using** context (let / temp / nested),
   for **any** opaque (`Fn`, `FnMut`, `Copy`, …) — not Fn-specific.
2. The **concrete-projection** witnesses the matrix/triage cited (`<u32 as Tr>::A`
   in return and `PhantomData` position) are **non-issues at HEAD** (P5/P6 = Ok);
   rustc pre-normalizes concrete projections before the bridge sees them. So
   RJ-013 is **purely** the opaque-reveal residue.
3. The fix is **viable**: the revealed hidden type of an `impl Fn`/`impl Copy`
   RPIT is a closure / primitive, and the bridge already bridges those (Q1/Q2 =
   Ok; `TyKind::Closure` arm at `bridge/ty.rs:382`; P2 = Ok). Revealing turns the
   `Alias` reject into the same `Ok` the defining fn already gets.

## 2. HEAD-grounded root cause (verified)

1. **The bridge's only alias-normalization is the A1 `AuthorizedProjectionFolder`,
   which matches `AliasTyKind::Projection` ONLY.** `translate_erased_ty`
   (`bridge/ty.rs:50-79`) calls `normalize_authorized_projections` (`:120-129`)
   before the inner `match`. The folder's `fold_ty` (`:140-156`) acts only on
   `TyKind::Alias(AliasTyKind::Projection, _)` nodes (and only the two authorized
   families — `Pointee::Metadata` + `FnOnceOutput`). It never touches
   `AliasTyKind::Opaque`.
2. **So an RPIT opaque falls through to the `TyKind::Alias(..)` short-circuit**
   at `bridge/ty.rs:438` → `non_rigid(NonRigidTyKind::Alias, ty)`.
3. **Opaques are revealable only in PostAnalysis / the defining scope** —
   `AliasTyKind::Opaque` doc: *"Can only be normalized away in PostAnalysis mode
   or its defining scope"* (`rustc_type_ir/src/ty_kind.rs:30-37`). This is why P2
   (opaque in `mk`'s own body = defining scope) bridges Ok while P1/P3/P4/Q4/Q5
   (opaque used in another body) reject: in a using body the opaque is rigid
   until PostAnalysis-revealed, and the bridge never reveals it.
4. **rustc reveals `ty::Opaque` under `TypingMode::PostAnalysis`** by instantiating
   `type_of(def_id)` (the hidden type) and recursing
   (`rustc_trait_selection/src/traits/query/normalize.rs:217-245`); outside
   PostAnalysis opaques are left rigid (`traits/normalize.rs:133`). The bridge runs
   `after_analysis` and the A1 folder already uses
   `TypingEnv::fully_monomorphized()` = `{ typing_mode: PostAnalysis, .. }`
   (`rustc_middle/src/ty/mod.rs:1014`). So the **same** `normalize_erasing_regions`
   call the folder already makes, applied to an `Opaque` node, reveals it.

## 3. The fix — reveal opaque alias nodes in the existing entry folder

Add an `AliasTyKind::Opaque` arm to `AuthorizedProjectionFolder::fold_ty`
(`bridge/ty.rs:140-156`) that reveals the opaque node via the same
`tcx.normalize_erasing_regions(TypingEnv::fully_monomorphized(), ty)` the
`Projection` arm uses, then continues folding the revealed type
(`normalized.fold_with(self)`) — mirroring the projection arm exactly, so a
nested opaque/projection inside the hidden type is handled (Q5).

**Mechanism (one node-local arm):**
```rust
fn fold_ty(&mut self, ty: Ty<'tcx>) -> Ty<'tcx> {
    match ty.kind() {
        TyKind::Alias(AliasTyKind::Projection, alias_ty)
            if is_authorized_projection(self.tcx, *alias_ty) =>
        { /* unchanged: normalize + fold_with */ }
        TyKind::Alias(AliasTyKind::Opaque, _) => {
            let revealed = self.tcx
                .normalize_erasing_regions(TypingEnv::fully_monomorphized(), ty);
            return revealed.fold_with(self);
        }
        _ => {}
    }
    ty.super_fold_with(self)
}
```
(Exact shape is the implementer's; the load-bearing points are: act on
`AliasTyKind::Opaque`, reveal node-locally with the PostAnalysis env, and
re-fold the result.)

Why **node-local** (not `normalize_erasing_regions` on the whole value): the A1
design (`bridge/ty.rs:108-119`) deliberately folds per-node so it does not widen
the *projection* acceptance set beyond the two authorized families. The opaque
arm keeps that property: it reveals every `Opaque` node but leaves
non-authorized `Projection` nodes to reject as before. Revealing **all** opaques
is correct and codegen-faithful (unlike projections, there is no "unauthorized
opaque" — every reachable RPIT in a complete monomorphic crate has a known
hidden type that codegen would use).

**Caveat (Codex):** rustc's PostAnalysis opaque reveal **recursively normalizes
aliases *inside* the hidden type** (`rustc_trait_selection/src/traits/query/normalize.rs:242-253`).
So the no-widening property holds for projection nodes **outside** or
**containing** the opaque (those still reject at `:438`), but a projection
embedded **inside** an opaque's revealed hidden type WILL be normalized as part
of the reveal. This is codegen-faithful (codegen sees the same revealed type) but
is broader than strict A1 node-locality — the spec must call it out, not claim
"no projection widening" unqualified.

**No model change. Bridge-only (one file, `bridge/ty.rs`).** `BRpo`/validator
invariants untouched.

## 4. Design forks (recommendation; Codex to pressure-test)

- **A. Reveal home.** *Recommend* extending the existing folder (one arm) —
  co-located with A1, reuses the same env + cache-consistency story. Alternative:
  a separate opaque-revealing pass (more code, splits the alias-normalization
  logic). Decide.
- **B. Folder/​fn rename.** The folder is named `AuthorizedProjectionFolder` /
  `normalize_authorized_projections`; adding opaque reveal broadens its role.
  *Recommend* a light rename (`AuthorizedAliasFolder` / `normalize_authorized_aliases`)
  + comment update, OR keep the name + a comment noting the broadened role
  (surgical). Decide — this touches A1 naming only.
- **C. Which alias kinds to reveal.** *Recommend* `Opaque` only (the witnessed
  gap). `AliasTyKind::Free` ("type alias … can always be normalized away",
  `ty_kind.rs:30-42`) is a candidate but is **not** surfaced by any probe — note
  as a Codex question (does a `Free` alias ever reach `translate_erased_ty`
  unresolved? likely rustc resolves it earlier). `Inherent` left untouched.
- **D. Reveal-failure handling.** `normalize_erasing_regions` **assumes success —
  it `bug!`s (panics) on a normalization failure** (`rustc_middle/src/ty/normalize_erasing_regions.rs:164-177`),
  exactly as the existing `Projection` arm already relies on. For a complete
  monomorphic `#![no_std]` crate post-`after_analysis`, every reachable RPIT has a
  known hidden type, so the reveal succeeds; a failure would be a rustc bug, not a
  case to defend against ([[feedback_no_defensive_fallbacks]]). Note the
  no-silent-mis-accept property: `revealed.fold_with(self)` re-enters the folder,
  so a (non-occurring) still-`Opaque` result would re-enter the Opaque arm — it
  would NOT silently slip past as a wrong type. So no defensive fallback is
  warranted or correct.

## 5. Soundness considerations (load-bearing → Codex soundness pass at assessment)

- **Forward/reverse cache consistency** — the A1 entry-normalization invariant
  (`bridge/ty.rs:63-68`): revealing at the entry keeps the `ty_forward_cache`
  (`TyId` ↔ revealed rustc `Ty`) in agreement so Phase-5 reverse resolution
  (`bridge/body/materialise.rs::translate_model_generic_args`) reads back the
  revealed type. The opaque arm reveals at the **same** entry point, so the same
  property holds — but **confirm no `Instance.generic_args` carries an opaque the
  reverse path expects un-revealed** (the witnesses don't, but probe a generic
  fn instantiated at an opaque arg if one is constructible).
- **Codegen faithfulness** — PostAnalysis reveal is exactly what codegen does
  (`query/normalize.rs:217`, "Only normalize `impl Trait` outside of type
  inference, usually in codegen"). The revealed closure/primitive is the type the
  C backend would see. Faithful.
- **Idempotence / no double-reveal hazard** — the revealed type has no remaining
  authorized-reveal node for that opaque; re-running the folder is a no-op on the
  revealed type.
- **No regression — WITH ONE CRITICAL COUPLING (see §9).** The reveal must NOT
  fire during **identity-signature** translation: the **RJ-009** covered row
  (`SignatureNonRigid` predicate) depends on a private/uncalled
  `fn opaque_adder() -> impl Fn(i32)->i32` whose *identity signature* rejects
  `NonRigidTy::Alias` and is absorbed by the bucket-4 carve-out
  (`signature_non_rigid=true`, `mod.rs:1837-1852`). Since `translate_fn_sig` →
  `translate_ty` → `translate_erased_ty` (the folder's entry), an UNCONDITIONAL
  reveal would make that signature succeed → no carve-out → RJ-009 fails + a
  spurious Instance appears → **Mode A regression**. The fix is therefore SCOPED
  to per-Instance body materialisation (§9). Concrete-projection rejection is
  unchanged (P5/P6 show rustc pre-normalizes them). With the scope, Mode A stays
  green and the new RJ-013 row is added (147 → 148).
- [[assessment-soundness-pass]]: a focused Codex soundness pass at assessment,
  since this changes the bridge's accepted type set for **every** opaque-bearing
  body.

## 6. Test plan sketch (for the spec)

- **Repro/positive (the RJ-013 row):** the body-RPIT witness (`let _g = mk()`
  where `mk() -> impl Fn() -> u32`) bridges `Ok` + validates; assert the revealed
  local is a closure (not an `Alias`). A second positive: `impl Copy` RPIT (P4) →
  Ok, to lock in that the fix is not Fn-specific.
- **Control:** the defining-fn-only case (P2) stays `Ok`; a bare closure local
  (Q1/Q2) stays `Ok` (no regression to the closure path the reveal targets).
- **Boundary (still rejects):** APIT-as-generic (Q3) stays `Ok` via the Param
  path (not the reveal); confirm a **non-authorized concrete-projection that
  genuinely survives** (if one is constructible) still rejects `Alias` — i.e. the
  fix does not widen projection acceptance. (If none is constructible from valid
  mono source, note that the `Alias` arm is now a defensive backstop, cf. the
  RJ-009/Param reframe.)
- **Reverse-path:** a witness exercising Phase-5 body materialisation of the
  revealed-opaque local (the let-binding witness already does; assert no ICE +
  validate()).
- **Regression:** Mode A `147/147/0` → `148/148/0` (every existing row already
  opaque-free → reveal is identity for them; the new RJ-013 row is the only delta).
- **Coverage row:** RJ-013 reframed `rejected`→`positive` **covered-by-reveal**
  (cf. FF-005's covered-by-rewrite), snippet = the body-RPIT witness, predicate =
  a positive bridge-Ok + (optionally) the revealed closure `Item`/`Instance`
  shape. Matrix RJ-013 `uncovered`→`covered`.

## 7. Scope / non-goals

- Fix is the opaque-reveal arm + its call (already in the folder) + the RJ-013
  row/test. **Bridge-only (`bridge/ty.rs`); no model change, no validator change,
  no `callbacks.rs` change.**
- **Not** the concrete-projection cases (P5/P6 = Ok at HEAD — non-issues; do NOT
  re-touch the A1 projection families).
- **Not** `async fn`/Future opaques, TAIT, RPITIT, or cross-crate TAIT reveal
  (out of `#![no_std]` driver scope; RPIT is the realized witness). If the
  implementer's probe finds one trivially covered, note it — do not expand scope.
- Reframing the `NonRigidTyKind::Alias` rejection: after the fix it remains a
  defensive backstop for non-authorized **projection** aliases (the A1 by-design
  boundary). We are NOT removing the `:438` arm.

## 8. Risks / open questions for Codex

1. **Reveal correctness** — does `normalize_erasing_regions(fully_monomorphized,
   opaque)` reveal the RPIT to its hidden type at the bridge's `after_analysis`
   call site? (Grounded: `ty_kind.rs:30-37` + `query/normalize.rs:217-245` +
   `mod.rs:1014`; empirically P2 + the defining-scope doc note. Confirm no
   `after_analysis`-vs-codegen phase gap.)
2. **Cache / reverse-resolution** — does revealing opaques at the entry ever
   desync the `ty_forward_cache` from what `translate_model_generic_args` expects
   on the reverse path? (A1 established the entry-normalization invariant for
   projections; confirm it transfers to opaques.)
3. **Node-local vs whole-value** — folding per-`Opaque`-node (reveal + re-fold) is
   sufficient for nested opaques (Q5) and preserves rejection for projections
   outside/containing the opaque. **Codex-resolved with a caveat:** the reveal
   recursively normalizes aliases *inside* the hidden type (`query/normalize.rs:242-253`),
   so projections inside a revealed hidden type are normalized too (codegen-faithful;
   see §3 caveat). The spec must state this rather than claim unqualified no-widening.
4. **`AliasTyKind::Free`** — can a `Free` alias reach `translate_erased_ty`
   unresolved (should it share the reveal), or does rustc always resolve it earlier?
5. **Reveal-failure** — **Codex-resolved:** `normalize_erasing_regions` assumes
   success and `bug!`s on failure (`normalize_erasing_regions.rs:164-177`); it does
   NOT return an unrevealed opaque. For valid mono source every reachable RPIT
   reveals, so this path is not hit; a failure would be a rustc bug. No defensive
   fallback (§4 Fork D).
6. **Backstop reachability** — after the fix, is `NonRigidTyKind::Alias` still
   reachable from valid mono source (some surviving non-authorized projection), or
   is it now purely defensive (RJ-009/Param reframe)? This determines whether the
   RJ-013 row is `positive` covered-by-reveal with the `Alias` arm kept as a
   documented backstop.

## 9. Spec-time finding (post-Codex): RJ-009 coupling → scope the reveal to body context

Investigating the test surface for the spec surfaced a coupling Codex's plan
review did not (it did not probe RJ-009). It changes the fix from "unconditional
reveal in the folder" (Fork A as written) to a **body-context-scoped reveal**.

**The coupling.** RJ-009 is a *covered* row whose witness is a private, uncalled
`fn opaque_adder() -> impl Fn(i32) -> i32` (`snippets/RJ-009.rs`). It is never
mono-collected, so `construct_function_instances` translates only its **identity
signature**: `translate_fn_sig(poly_fn_sig)` (`bridge/mod.rs:1832-1837`) →
`translate_ty` → `translate_erased_ty`. The RPIT return is `Alias(Opaque)`, which
rejects `NonRigidTy`, and the **bucket-4 carve-out** absorbs it by setting
`BridgeItemFlags.signature_non_rigid = true` and pushing no Instance
(`bridge/mod.rs:1839-1852`). The `SignatureNonRigid` inspect predicate
(`inspect.rs:101-111`) observes that flag — its doc even names this exact shape.

**Why an unconditional reveal breaks it.** Both the identity-signature path
(`mod.rs:1837`) and per-Instance body materialisation (`materialise.rs:304`,
`translate_ty(cx, local.ty)`) flow through the SAME `translate_erased_ty` entry
where the folder runs. An unconditional opaque-reveal would make
`opaque_adder`'s identity signature SUCCEED (revealed to its closure) → the
`Err(NonRigidTy)` carve-out arm is never hit → `signature_non_rigid` stays false
→ RJ-009's predicate finds nothing → **Mode A regresses**, and `opaque_adder`
gains a spurious Instance (`mod.rs:1860`).

**The scope (the real design).** Reveal opaques ONLY while materialising a
concrete per-Instance body, NOT during identity-signature translation. This is
also the faithful distinction: a body local at a concrete use site has a
demanded, known reveal (RJ-013); an uncalled fn's polymorphic identity signature
has no concrete use and legitimately keeps its opaque (RJ-009). Mechanism options:

- **Recommended — transient `BridgeCtxt` flag (`reveal_opaque_types`, default
  false).** Set `true` for the duration of `materialise_function_body_inner`
  (locals + statements + terminators + their generic args — the whole body walk),
  restore after. `translate_erased_ty` reads it and passes it into the folder
  (the folder gains a `reveal_opaques: bool`; the `Projection` arm is UNCHANGED).
  Comprehensive: reveals every body-context opaque (covers Q4 temp-into-call,
  where the opaque also rides a Call's generic args, not only a `let` decl), and
  is OFF for the `mod.rs:1837` identity-sig path by default → RJ-009 preserved.
  The folder/​fn rename (Fork B) becomes natural: `AuthorizedAliasFolder` /
  `normalize_authorized_aliases`.
- **Alternative — surgical reveal of body local-decl types only** (a
  `reveal_opaques(tcx, ty)` helper called at `materialise.rs:304` before
  `translate_ty`). No folder/ctx change; RJ-009-safe by construction. But it
  reveals only local-decl types — an opaque surfaced solely via a Call's generic
  args inside the body (plausibly Q4) could still reject. Less complete; prefer
  the flag unless the implementer's probe shows local decls are exhaustive for
  every witness.

**Mandatory test (either mechanism):** RJ-009 must stay `covered`
(`SignatureNonRigid` still passes) AND `opaque_adder` must remain zero-Instance —
add this as an explicit regression assertion + keep RJ-009 in Mode A (148/148/0).

**Fork A is thus refined:** still extend the existing folder (cohesion), but gate
the new `Opaque` arm on the body-context flag rather than revealing
unconditionally. Forks B/C/D and §3's node-local + recursive-hidden-type caveat
are unchanged.
