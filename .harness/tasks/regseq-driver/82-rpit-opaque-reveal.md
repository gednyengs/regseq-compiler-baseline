# regseq-driver-82 — body-context reveal of RPIT opaque types (RJ-013: `NonRigidTy{Alias}` residue)

## 1. Description

A body that **uses a value whose type is a return-position `impl Trait` (RPIT)
opaque from a callee** fails to bridge:

```rust
#![no_std]
pub fn mk() -> impl Fn() -> u32 { || 0 }
pub fn f() -> u32 { let g = mk(); g() }
```

At HEAD: `BridgeOutcome::Rejected(NonRigidTy { kind: Alias, ty_repr: "impl Fn() -> u32" })`.
The opaque bridges fine in its **defining** fn (`mk` alone = `Ok`) and a bare
closure-typed local is `Ok`; only an opaque **used** in another body rejects.

Root cause (verified; full probe table + Codex review in
`.harness/audits/2026-05-25-rj013-opaque-reveal-plan.md`): the bridge's only
alias-normalization is the driver-70 A1 `AuthorizedProjectionFolder`, which
matches `AliasTyKind::Projection` ONLY (`bridge/ty.rs:140-156`). An RPIT
`AliasTyKind::Opaque` is never revealed, so it falls to the `TyKind::Alias(..)`
short-circuit (`bridge/ty.rs:438`) → `NonRigidTyKind::Alias`. rustc reveals
`ty::Opaque` to its hidden type only under `TypingMode::PostAnalysis`
(`rustc_type_ir/src/ty_kind.rs:30-37`; `rustc_trait_selection/src/traits/query/normalize.rs:217-245`),
which `TypingEnv::fully_monomorphized()` provides (`rustc_middle/src/ty/mod.rs:1014`).

Fix: reveal `AliasTyKind::Opaque` nodes via the SAME
`normalize_erasing_regions(fully_monomorphized, ty)` the A1 projection arm
already uses — but **scoped to per-Instance body materialisation** (see §3 / the
plan §9 RJ-009 coupling). The revealed hidden type (a closure / primitive) is
already bridgeable (`TyKind::Closure` arm, `bridge/ty.rs:382`), so the reject
becomes the `Ok` the defining fn already gets. **Bridge-only; no model change; no
validator/`callbacks.rs` change.**

## 2. Input Context

### Design source

- `.harness/audits/2026-05-25-rj013-opaque-reveal-plan.md` — Codex-APPROVED plan
  (+ §9 spec-time finding). Probe table (P1-P7, Q1-Q5) establishes the gap is
  exactly body-context RPIT opaque-use (any opaque, not Fn-specific); concrete
  projections (P5/P6) and APIT args (Q3) are non-issues at HEAD.

### Bridge anchors

- `crates/regseq-driver/src/bridge/ty.rs:50-79` — `translate_erased_ty` entry
  (runs `normalize_authorized_projections` before the inner `match`); `:120-156`
  the folder (`normalize_authorized_projections` / `AuthorizedProjectionFolder` /
  `is_authorized_projection`); `:382` `TyKind::Closure` arm (the revealed hidden
  type lands here); `:438` `TyKind::Alias(..) => non_rigid(Alias)`; `:836`
  `fn non_rigid`. Imports at `:3-7` already include `AliasTyKind`, `TypeFolder`,
  `TypeFoldable`, `ty` — no new import needed.
- `crates/regseq-driver/src/bridge/ty.rs:26-31` — `translate_ty` (erases then
  calls `translate_erased_ty(.., false)`); used by BOTH body local translation
  AND identity-signature translation — the shared entry that makes scoping
  necessary.
- `crates/regseq-driver/src/bridge/body/materialise.rs:57` —
  `materialise_function_body_inner` (the per-Instance body builder; monomorphises
  the body via `instance.args` per driver-60, `:90-108`); `:303-304` local-decl
  loop `translate_ty(cx, local.ty)`. **The body-context scope is set here.**
- `crates/regseq-driver/src/bridge/mod.rs:1832-1855` — the bucket-4 carve-out:
  identity signature `translate_fn_sig(poly_fn_sig)` → on `Err(NonRigidTy)` sets
  `BridgeItemFlags.signature_non_rigid = true` and pushes no Instance. **This path
  MUST keep rejecting the opaque** (RJ-009; see §4).
- `crates/regseq-driver/src/bridge/fnsig.rs:23-42` — `translate_fn_sig` (calls
  `translate_ty` per input/output) — the identity-sig path that must NOT reveal.
- `BridgeCtxt` definition (search `struct BridgeCtxt` in `bridge/mod.rs`) — gains
  the transient reveal flag.

### Prior tasks

- driver-70 (A1, `4c1332e`) — the `AuthorizedProjectionFolder` this extends;
  established the entry-normalization cache-consistency invariant (`ty.rs:63-77`).
- driver-72 (F72, `9186248`) — added the `SignatureNonRigid` predicate +
  bucket-4 carve-out coverage (RJ-009). **The coupling this task must preserve.**
- driver-80 triage — RJ-013 is the MEDIUM Bucket-B item; "normalize-or-adjudicate"
  → architect chose the normalize fix.

### Test surface

- New `crates/regseq-driver/tests/bridge_rpit_opaque_reveal.rs` (end-to-end +
  the RJ-009 regression guard).
- New `crates/bridge-coverage-runner/snippets/RJ-013.rs` + `rows.toml` row
  (reframed `rejected`→`positive`/`covered`) + matrix `RJ-013` status; Mode A.

## 3. Implementation Notes

### Change A — body-context-gated opaque reveal in the folder (`bridge/ty.rs`)

1. Add a transient flag to `BridgeCtxt`: `reveal_opaque_types: bool` (default
   `false` at construction).
2. In `translate_erased_ty` (`ty.rs:69`), read the flag and thread it into the
   normalization: `let ty = normalize_authorized_aliases(cx.input.tcx, ty, cx.reveal_opaque_types);`
3. Rename `normalize_authorized_projections` → `normalize_authorized_aliases`
   (Fork B; broadened role) taking a `reveal_opaques: bool`; give the folder a
   `reveal_opaques: bool` field. Add an arm to `fold_ty`:
   ```rust
   TyKind::Alias(AliasTyKind::Opaque, _) if self.reveal_opaques => {
       let revealed = self.tcx
           .normalize_erasing_regions(TypingEnv::fully_monomorphized(), ty);
       return revealed.fold_with(self);
   }
   ```
   The existing `Projection` arm is UNCHANGED (still runs regardless of the flag).
   Keep the `has_aliases()` fast-path guard.

### Change B — set the body-context scope, RAII-guarded (`bridge/`)

The flag must be `true` for the duration of **every per-Instance body-derived
type translation**, restored on exit. Use an **RAII guard** (a small struct that
saves the prior value on construction and restores it on `Drop`) — NOT a bare
`set true … set false`, because the body walk uses `?` and can lazily create
further instances mid-translation (`bridge/body/terminator.rs:950`), so an early
return must still restore the flag.

Set the guard at BOTH body-derived sites (Codex spec review — the second is the
gap a materialise-only scope misses):
1. `materialise_function_body_inner` (`materialise.rs:57`) — wraps the local-decl
   loop (`:303-304`) and the body walk (statements/places/rvalues/terminators all
   route through `translate_ty`: `body/place.rs:36`, `body/rvalue.rs:49`,
   `body/terminator.rs:831,937`).
2. `construct_closure_call_instances` (`bridge/mod.rs:2349` defn; signature
   input/output translation at `:2490-2503`) — closure-call instance construction
   is **body-derived but runs before `run_phase5_body_passes`** (`mod.rs:1547-1556`)
   and translates closure signature input/output types **outside** site 1's
   window. Without the guard here, a closure whose signature carries an RPIT
   opaque (e.g. `let c = || mk(); c()`) would still reject.

The identity-signature path (`construct_function_instances`, `mod.rs:1837`
`translate_fn_sig`) is OUTSIDE both guards → flag stays `false` → the opaque is
still rejected → the RJ-009 bucket-4 carve-out is preserved. The implementer must
**grep for any other per-Instance body-derived `translate_ty`/`translate_erased_ty`
site** and confirm none carries a body-context opaque left unrevealed (extend the
guard if so); the B-test below is the empirical check.

(Alternative considered + rejected as incomplete — plan §9: revealing only
local-decl types at `materialise.rs:304` — misses both Call generic-args and the
closure-signature site.)

### Change C — coverage row + tests

- New `snippets/RJ-013.rs`: the body-RPIT **call** witness
  (`mk() -> impl Fn() -> u32`; `f() -> u32 { let g = mk(); g() }`). `rows.toml`
  `RJ-013` row reframed `expectation = "positive"`, `status = "covered"`,
  predicate **`{ kind = "SyntheticItemKind", arg = "ClosureCall" }`** (matches the
  8 existing closure-call rows). **Coverage-gating rationale (Codex r2):** neither
  `ClosureCall` nor `TyVariant=Closure` *isolates* `f`'s reveal — `ClosureCall` is
  synthesized from closure-type *presence* (`synth/closure.rs:48-58`), so `mk`'s
  defining closure already satisfies it. The real discriminator is the **positive
  bridge outcome**: at HEAD the whole crate Rejects on `f`'s opaque (so the row
  FAILS — expected `positive`, actual `rejected`); post-fix it bridges `Ok` and the
  predicate confirms the closure-call construct is present. The **precise** "f's
  opaque revealed to a closure, no `Alias`" evidence is the load-bearing
  integration test B1/B2 — the runner row gates the outcome, the test carries the
  proof. Matrix `RJ-013` `uncovered`→`covered` (covered-by-reveal, cf. FF-005
  covered-by-rewrite).
- `bridge_rpit_opaque_reveal.rs` integration test (see Test Plan).

## 4. Non-Goals

- **RJ-009 MUST stay covered** — do NOT make identity-signature translation reveal
  opaques. `opaque_adder`'s signature must still reject `NonRigidTy`, set
  `signature_non_rigid=true`, and push no Instance (`mod.rs:1839-1852`). This is
  the load-bearing scope constraint, not an optional nicety.
- **No concrete-projection retouch** — P5/P6 bridge `Ok` at HEAD; do not alter the
  A1 `Projection` arm or its two authorized families.
- **No model/validator/`callbacks.rs` change** — the `NonRigidTyKind::Alias` arm
  (`ty.rs:438`) stays as a defensive backstop for surviving non-authorized
  projection aliases.
- **Out of scope:** `async fn`/Future opaques, TAIT, RPITIT, cross-crate TAIT
  reveal (not surfaced by any `#![no_std]` driver witness). If one is incidentally
  covered, note it — do not expand scope.

## 5. Test Plan

### Bridge integration tests (`bridge_rpit_opaque_reveal.rs`)

- **B1 let-form reveal**: `mk() -> impl Fn() -> u32`; `f() { let g = mk(); … }`
  bridges `Ok` + validates; assert a `Ty::Closure` is present and NO body/Item
  carries an `Alias` (the opaque was revealed, not rejected).
- **B2 call-form**: `f() -> u32 { let g = mk(); g() }` bridges `Ok`; assert a
  `SyntheticItemKind::ClosureCall` is present (the revealed closure is invoked).
- **B3 non-Fn opaque**: `mk() -> impl Copy { 5u32 }`; used in a body → `Ok`
  (reveals to `u32`; locks in that the fix is not Fn-specific).
- **B4 nested RPIT** (Q5): `outer()` returns `inner()`'s opaque; used in a body →
  `Ok`.
- **B4b closure-signature RPIT (body-derived; Codex)**: a closure whose signature
  carries an RPIT opaque — e.g. `pub fn f() -> u32 { let c = || mk(); let g = c(); g() }`
  with `mk() -> impl Fn() -> u32` — bridges `Ok`. This exercises Change B **site 2**
  (`construct_closure_call_instances`); without that guard it would still reject.
  If the shape proves not constructible/reachable, the implementer documents why
  and scopes it out explicitly (do not leave it silently failing).
- **B5 controls (no regression to the reveal target)**: the defining-fn-only case
  (`mk` alone) stays `Ok`; a bare closure local (`let g = || 0u32; g()`) stays
  `Ok`.
- **B6 RJ-009 REGRESSION GUARD (load-bearing)**: the exact RJ-009 witness
  (`fn opaque_adder() -> impl Fn(i32)->i32 { |x| x+1 }` private/uncalled +
  `pub fn entry() -> i32 { 0 }`) still bridges `Ok` AND `opaque_adder`'s Item has
  `signature_non_rigid == true` AND has **zero** Instances. (Assert via
  `model.item_bridge_flags` + the instance count for that Item.) This proves the
  reveal did NOT leak into identity-signature translation.

### Mode A

- **B7**: full runner; `Total == Pass`, `Fail == 0`, exit 0, with the reframed
  `RJ-013` row (147 → 148) AND `RJ-009` still PASS (`SignatureNonRigid`); every
  other pre-existing row unchanged.

## 6. Review

### Files to send to Codex

- This spec + the plan (esp. §9) + the implemented diff (Change A/B/C).

### Reviewer focus

1. **SCOPE / RJ-009 coupling (PRIMARY):** does the reveal fire during per-Instance
   body-derived translation (both `materialise_function_body_inner` AND
   `construct_closure_call_instances`) and NEVER during identity-signature
   translation (`mod.rs:1837`)? Is the flag RAII-restored on `?`/early-return (no
   leak across items)? Does RJ-009 stay covered (`signature_non_rigid` still set,
   `opaque_adder` zero-Instance)? Are there other body-derived `translate_ty`
   sites left unguarded?
2. **Reveal correctness:** `normalize_erasing_regions(fully_monomorphized,
   opaque)` reveals to the hidden type at this `after_analysis` call site; the
   revealed closure/primitive bridges via the existing arms.
3. **Cache/reverse-path:** revealing at the entry keeps `ty_forward_cache`
   consistent for Phase-5 reverse resolution (`materialise.rs` `translate_model_generic_args`),
   as the A1 invariant established for projections.
4. **No widening:** projections outside/containing the opaque still reject
   (`:438`); acknowledge the reveal recursively normalizes aliases INSIDE the
   hidden type (plan §3 caveat).
5. **No regression:** Mode A 148/148/0; `Projection` arm + invariants unchanged.

### Adversarial prompt template (four-block)

```
<task>
Review the implemented diff for regseq-driver-82 (cwd = repo root): a body-context-scoped
reveal of RPIT AliasTyKind::Opaque types fixing the RJ-013 NonRigidTy{Alias} reject, per the
APPROVED plan .harness/audits/2026-05-25-rj013-opaque-reveal-plan.md (esp. §9). A transient
BridgeCtxt flag (RAII-guarded) gates an opaque-reveal arm in the A1 alias folder; the flag is set for
per-Instance body-derived translation at materialise_function_body_inner AND
construct_closure_call_instances. THE PRIMARY RISK is that the reveal leaks into
identity-signature translation (bridge/mod.rs:1837) and breaks the RJ-009 bucket-4
signature_non_rigid carve-out (regressing Mode A). Verify: the reveal is body-scoped and
RAII-restored on early-return; RJ-009 stays covered (signature_non_rigid still true, opaque_adder zero-Instance);
the revealed hidden type bridges; ty_forward_cache stays consistent; projections outside the
opaque still reject; Mode A is 148/148/0. Build, run cargo test + Mode A, and probe the body-RPIT,
impl Copy, and RJ-009 witnesses.
</task>
<grounding_rules>
Cite file:line. Run the tests + Mode A + probes. Confirm the flag cannot be true during the
mod.rs:1837 identity-sig path. This is a load-bearing type-acceptance change — be adversarial
about the RJ-009 coupling and any opaque the reveal now accepts that it should not.
</grounding_rules>
<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
REVEAL_BODY_SCOPED_NO_SIG_LEAK: PASS|FAIL + cite
RJ009_STILL_COVERED: PASS|FAIL + cite
REVEAL_CORRECT_AND_BRIDGEABLE: PASS|FAIL + cite
CACHE_REVERSE_PATH_SAFE: PASS|FAIL + cite
NO_REGRESSION_MODE_A_148: PASS|FAIL + cite
NEW ISSUES: numbered or "none"
REQUIRED FIXES: numbered (only if VERDICT != APPROVED)
</structured_output_contract>
<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## 7. Quality Gate

- `cargo build` clean.
- `cargo fmt --all -- --check` clean.
- `cargo clippy -p regseq-driver --lib -- -D warnings` + the new test target
  clean. **Do NOT gate on `--all-targets`** ([[clippy-all-targets-test-drift]]).
- `cargo test -p regseq-driver -- --test-threads=1` green (incl.
  `bridge_rpit_opaque_reveal.rs`); `cargo test -p regseq-model` green (unchanged).
- Mode A: `Total == Pass`, `Fail == 0`, exit 0, with `RJ-013` covered (147 → 148)
  AND `RJ-009` still PASS.
- **A focused Codex soundness pass is REQUIRED at assessment**
  ([[assessment-soundness-pass]]): this changes the bridge's accepted type set for
  opaque-bearing bodies; the load-bearing risk is the RJ-009 identity-sig leak
  (B6 is the primary guard).
- Diff scope confined to: the `bridge/ty.rs` folder arm + flag plumbing; the
  `BridgeCtxt` flag field + the RAII guard; the two RAII scope sites
  (`materialise_function_body_inner` + `construct_closure_call_instances`);
  `RJ-013` snippet + row + matrix status; the new integration test. No
  model/validator/`callbacks.rs` change; no A1 `Projection`-arm change; no other
  rows.

## 8. Acceptance Criteria

1. The body-RPIT witness (`let g = mk(); g()`, `mk() -> impl Fn() -> u32`) bridges
   `Ok` + validates; the opaque is revealed to a `Ty::Closure` (no `Alias`).
   `impl Copy` (B3) and nested RPIT (B4) also bridge `Ok`.
2. **RJ-009 stays covered:** `opaque_adder`'s identity signature still rejects
   `NonRigidTy` → `signature_non_rigid == true`, zero Instances; the
   `SignatureNonRigid` predicate still passes. The reveal does NOT fire during
   identity-signature translation.
3. The reveal is gated on a transient `BridgeCtxt` flag, **RAII-guarded** (restored
   on `Drop`, exception-safe across `?`), set for per-Instance body-derived
   translation at BOTH `materialise_function_body_inner` AND
   `construct_closure_call_instances` — never during identity-signature
   translation (no cross-item leak).
4. The A1 `Projection` arm + the two authorized families are unchanged; the
   `NonRigidTyKind::Alias` backstop (`ty.rs:438`) is unchanged.
5. No model / validator / `callbacks.rs` change.
6. Mode A `148/148/0` exit 0 (`RJ-013` covered, `RJ-009` still PASS, all other
   rows unchanged); build / fmt / clippy (lib + new test) / `cargo test -p
   regseq-driver` / `-p regseq-model` green.
7. Diff confined per the Quality Gate.
