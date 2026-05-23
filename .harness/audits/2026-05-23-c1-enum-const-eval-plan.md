# regseq-driver-73 plan: C1 — const-eval for unevaluated ADT-enum consts (OP-003-I)

**Status**: NOT_READY — BLOCKED ON SCOPE/DESIGN DECISION (2026-05-23).
Codex review (session `019e5699`) returned NOT_READY and showed the
plan's core premise (fix-hook in `translate_const`) is WRONG. The real
blocker is verified at HEAD:
- The bridge **deliberately rejects ALL const-generic arguments** at
  the forward boundary `translate_generic_args`
  (`crates/regseq-driver/src/bridge/ty.rs:743-748`) and the backward
  boundary `translate_model_generic_args`
  (`crates/regseq-driver/src/bridge/body/materialise.rs:673-684`) — a
  coordinated gap tied to regseq-driver-37, not an enum-const-eval gap.
- `Const::EnumValue` is **deliberately excluded** from type-level
  admissibility; only scalar consts (Bool/Char/Int/Uint/Float) may be a
  `GenericArg::Const` (`crates/regseq-model/src/const_value.rs:54-64`).
- The F4 error string (`UnevaluatedConst {...}`) is therefore NOT stale
  — it is the live output of the `ty.rs:744-747` rejection
  (`format!("{konst:?}")`).

OP-003-I is thus entangled with broad const-generic support + a model
admissibility design decision, NOT an isolated const-eval fix as the
F68 audit billed. The §3-§8 sketch below is retained for context but
its fix-hook is wrong.

**RESOLUTION (2026-05-23): OPTION (1) — RECLASSIFY, no code.** The
architect decided enum/non-scalar const-generics are out of scope for
`#![no_std]` driver specs and the scalar-only type-level admissibility
(`const_value.rs:54-64`) is intentional. OP-003-I is therefore not an
achievable positive row; it is reclassified `uncovered` →
`blocked-bridge-rejects-by-design` in the matrix
(`docs/discussions/regseq-model/bridge-coverage-matrix.md` OP-003-I
row), cross-referencing Section 6 RJ-029 (`UnsupportedConst`). No
bridge/model code change. C1 is closed-by-reclassification; the F68
audit's C1 proposal is retired. Const-generic support broadly remains a
deferred gap coordinated with driver-37, to be taken up deliberately if
and when const-generics become a driver-spec requirement (not via this
proposal).

**Source**: F68 audit Cluster (C), proposal C1, at
`.harness/audits/2026-05-21-post-f4-triage.md:272-310`. Audit
Closing-recommendations order: C1 is MEDIUM, next after the
already-closed D1 (F69) + A1 (F70). Unblocks the single escalated row
OP-003-I.

## 1. Problem statement (verified at HEAD 9186248)

OP-003-I's matrix construct is `Const::EnumValue { variant_idx,
payload }` (`docs/discussions/regseq-model/bridge-coverage-matrix.md:200`,
status `uncovered`, 0 sidecar entries). The goal: get the bridge to
emit a `Const::EnumValue` for an enum constant used in a
const-generic position.

The producing path already exists: `translate_ty_const_value`'s
ADT-enum branch at `crates/regseq-driver/src/bridge/const_.rs:91-103`
destructures an enum value-tree into `Const::EnumValue { variant_idx,
payload }`. It is reached from `translate_const`'s
`mir::Const::Ty(_, ty_const)` branch at
`crates/regseq-driver/src/bridge/const_.rs:36-40` — but ONLY when
`ty_const.try_to_value()` returns `Some`.

For the OP-003-I shapes (`pub const V: E = E::A(7); W<V>`, and
`W<{ E::A(7) }>` with `adt_const_params`), the const-generic argument
arrives as an **unevaluated** `ty::Const` (`ConstKind::Unevaluated`).
So `try_to_value()` returns `None` (`const_.rs:37`), the
`mir::Const::Ty` branch does NOT route to `translate_ty_const_value`,
and control falls through to `konst.eval(...)` at
`crates/regseq-driver/src/bridge/const_.rs:43-47`, which fails and
raises `BridgeError::UnsupportedConst`. The existing `Const::EnumValue`
path at `:91-103` therefore never executes.

**STALE-EVIDENCE WARNING.** The F4 escalation record at
`.harness/expectations/regseq-driver-67.md:222-236` quotes the error
as `UnsupportedConst { const_repr: "UnevaluatedConst { def:
f::{constant#0}, args: [] }" }`. That string predates const_.rs
changes and does NOT match HEAD's current error format at
`const_.rs:45-46` (`"const eval failed for {konst:?}: {error:?}"`).
The implementer MUST empirically re-reproduce the failure against HEAD
and pin the *current* carrier (`mir::Const::Ty` with an unevaluated
`ty_const`, vs `mir::Const::Unevaluated`, vs something else) and the
*current* error before writing any fix. This is the F72 lesson: do not
trust a stale escalation record's premise; verify against current
code. (F72 sub-task C's spec premise was false; the implementer caught
it empirically.)

## 2. Code-surface grounding

- `crates/regseq-driver/src/bridge/const_.rs:32-49` — `translate_const`
  entry: the `mir::Const::Ty` → `try_to_value` → `translate_ty_const_value`
  fast path (`:36-40`), then the `konst.eval(...)` fallback (`:43-47`).
- `crates/regseq-driver/src/bridge/const_.rs:51-122` —
  `translate_ty_const_value`: the value-tree → `Const` translator,
  including the ADT-enum branch at `:91-103` that produces
  `Const::EnumValue`.
- `crates/regseq-driver/src/bridge/const_.rs:124-...` —
  `translate_ty_const`: recursive child translator (also gated on
  `try_to_value`, `:128`).
- `crates/regseq-model/src/const_value.rs:41-47` — the
  `Const::EnumValue { variant_idx, payload }` model shape (payload
  shape confirmed `{ variant_idx, payload }` per matrix v2.1 change
  log, NOT `{ variant, payload, ty }`).
- **No existing precedent**: a grep of `crates/regseq-driver/src/bridge/`
  for `const_eval_resolve` / `eval_to_valtree` / `try_to_valtree` /
  `ConstKind::Unevaluated` finds only the two `try_to_value()` call
  sites. C1 introduces the first explicit unevaluated-const evaluation
  in the bridge — there is no internal pattern to mirror (unlike F72
  sub-task B's `lookup_or_create_direct_instance` precedent).
- Runner predicate: `const_variant_tag` returns `"EnumValue"` for
  `Const::EnumValue { .. }` at
  `crates/bridge-coverage-runner/src/inspect.rs:234`, dispatched by
  `InspectionPredicate::ConstVariant` at `:674-684`. So OP-003-I's
  row uses `inspect = { kind = "ConstVariant", arg = "EnumValue" }`.

## 3. Proposed scope

Extend the const-translation entry so an unevaluated type-level enum
const is evaluated to a value-tree, then routed through the existing
`translate_ty_const_value` ADT-enum path. Two admissible directions;
the implementer chooses after empirical reproduction:

- **Path 3.a (RECOMMENDED) — evaluate the unevaluated `ty::Const` to a
  value-tree, then reuse the existing path.** In the `mir::Const::Ty(_,
  ty_const)` branch, when `try_to_value()` returns `None` because the
  const is `ConstKind::Unevaluated`, resolve it to a `ty::Value` /
  valtree via the appropriate rustc API, then hand the result to
  `translate_ty_const_value`. Candidate APIs (the implementer pins the
  exact one against the pinned nightly rustc source — see Risks):
  - `tcx.const_eval_resolve_for_typeck(typing_env, unevaluated, span)`
  - `ty_const.eval(tcx, typing_env, span)` followed by `try_to_value()`
  - `tcx.const_eval_resolve(...)` + valtree construction
  This is preferred because the `Const::EnumValue` producer at `:91-103`
  already exists and is correct; the only missing step is producing the
  valtree it consumes. Minimal new surface.

- **Path 3.b — coax MIR const folding.** Drive the unevaluated const
  through MIR-level const evaluation (`generic_const_exprs` / lazy
  normalization) so `konst.eval(...)` at `:43-47` succeeds and yields a
  value the downstream `translate_const_value` consumes. Larger,
  fuzzier surface; only if 3.a proves infeasible.

**Decision discipline**: the implementer MUST (i) empirically reproduce
the HEAD failure and record the current carrier + error, (ii) choose
3.a or 3.b with rationale, (iii) verify the chosen rustc API against
the pinned nightly rustc source, and (iv) document all of this in the
expectation file.

## 4. Snippet requirements (OP-003-I.rs)

A fresh snippet at `crates/bridge-coverage-runner/snippets/OP-003-I.rs`
exercising the `pub const V: E = E::A(7); W<V>` shape:

- Likely needs `#![feature(adt_const_params)]` (and possibly
  `#![feature(unsized_const_params)]` depending on the pinned nightly)
  to use an enum as a const-generic parameter type.
- The enum `E` must derive / implement the const-generic-param marker
  trait (`ConstParamTy` / `ConstParamTy_`, plus `PartialEq`, `Eq`, and
  whatever the pinned nightly requires).
- A struct/type `W<const N: E>` and a use site producing the enum
  const in const-generic position.
- The exact feature gates + derives MUST be determined empirically
  against the pinned nightly (`adt_const_params` requirements have
  shifted across nightlies). This mirrors F70/A1 needing
  `#![feature(ptr_metadata)]`.
- `#![no_std]` per the spec environment.
- Predicate: `inspect = { kind = "ConstVariant", arg = "EnumValue" }`.
- Expectation class: `positive` (`bridge_ok` + a model carrying a
  `Const::EnumValue`).

## 5. Acceptance criteria (preview)

- **AC1**: HEAD failure empirically reproduced; current carrier + error
  recorded in the expectation file (superseding the stale F4 string).
- **AC2**: chosen path (3.a / 3.b) + the exact rustc const-eval API
  documented, with verification against the pinned nightly rustc
  source.
- **AC3**: OP-003-I snippet authored; the bridge emits a
  `Const::EnumValue { variant_idx, payload }` for it; the
  `ConstVariant=EnumValue` predicate PASSes.
- **AC4**: OP-003-I committed to sidecar; Mode A `Total = 141`,
  `Pass = 141`, `Fail = 0`.
- **AC5**: the existing 140 sidecar rows remain green (no regression);
  in particular the other const paths (Bytes / Aggregate / Reloc /
  leaf scalars) are unchanged.
- **AC6**: OP-003-I matrix row status refreshed `uncovered` → `covered`
  with the snippet as `fixture_path`. (Only this row; the broader
  matrix↔sidecar drift is out of scope — see §6.)

## 6. Out of scope

- **B1 (FF-012 CloneShim) and C2 (FF-013 FnPtrAddrShim).** The F68 audit
  Closing note (`:519-535`) warns B1/C1/C2 share the const-translation
  surface; C1 lands FIRST per audit order, so there is no coordination
  conflict now. C1 must NOT pre-emptively refactor for B1/C2.
- **The matrix↔sidecar coverage drift** (`.harness/audits/2026-05-23-matrix-sidecar-drift.md`).
  C1 backfills only its own OP-003-I row (AC6), exactly as F71/F72 did
  for their rows. The systemic 121-row drift is a Category 5 derived-
  reconciliation job, NOT this task.
- **The `Const::EnumValue` model shape** (`const_value.rs:41-47`) —
  unchanged. C1 produces it, does not redefine it.
- **`translate_const_value`** (the non-`Ty` const path) — unchanged
  unless path 3.b is chosen AND the implementer documents why.
- **Other const variants** (Reloc, NullPtr, Float, etc.) — out of scope.
- **F70's `AuthorizedProjectionFolder`** and any non-const bridge
  surface — untouched.
- **No `task.py add-task`** for any new escalation; **no commits/pushes
  without user authorization**; **no `tests/fixtures/playground*`
  edits** (standing rules).

## 7. Risks

- **R-1 (unstable feature surface)**: `adt_const_params` is an unstable
  nightly feature whose requirements (marker trait name, required
  derives, companion feature gates) have changed across nightlies. The
  snippet may not compile first-try. *Mitigation*: AC1 requires
  empirical reproduction; the implementer iterates the snippet against
  the pinned `nightly-2026-02-01` until it both compiles and surfaces
  the unevaluated-enum-const carrier.
- **R-2 (rustc const-eval API uncertainty)**: there is no bridge
  precedent for evaluating an unevaluated `ty::Const` to a valtree, and
  the exact API (`const_eval_resolve_for_typeck` vs `ty_const.eval` vs
  `const_eval_resolve`) + its `TypingEnv` requirements must be pinned
  against the pinned rustc source. *Mitigation*: AC2 + the rustc-source
  verification requirement (the rustup-pinned rustc source is available
  locally).
- **R-3 (valtree shape mismatch)**: the evaluated valtree must be the
  same `ty::Value` shape the existing `:91-103` ADT-enum branch
  consumes (`destructure_adt_const` → `variant` + `fields`). If
  `const_eval_resolve_for_typeck` returns a raw `ConstValue` rather than
  a valtree, an extra conversion step is needed. *Mitigation*: AC3
  empirically verifies the produced `Const::EnumValue`.
- **R-4 (bounded-gate analogue)**: if BOTH paths 3.a and 3.b prove
  infeasible against the pinned nightly (e.g., the feature is too
  unstable to produce a clean witness), the implementer escalates C1
  with rationale rather than forcing a fix — sidecar stays at 140 and
  OP-003-I stays escalated. (This mirrors F72 sub-task C's bounded gate.)

## 8. Open questions for Codex

- **Q1**: is path 3.a's "evaluate the unevaluated ty_const, then reuse
  `translate_ty_const_value`" the right primary direction, or is there
  a reason MIR-level folding (3.b) is more spec-faithful per spec.md
  §9.7?
- **Q2**: are the code-surface citations accurate at HEAD? In
  particular, does the `mir::Const::Ty` branch at `:36-40` actually
  receive the const-generic enum const, or does it arrive as
  `mir::Const::Unevaluated` (a different `mir::Const` variant) that
  bypasses the `Ty` branch entirely? (This changes where the fix
  hooks in.)
- **Q3**: is the snippet feature-gate guidance (`adt_const_params`,
  `ConstParamTy`) correct for `nightly-2026-02-01`, or should the plan
  leave it fully to empirical determination?
- **Q4**: is the single-row matrix backfill (AC6) the right scope given
  the known systemic drift, or should C1 say nothing about matrix
  status to avoid implying the matrix is reliable?
- **Q5**: is the bounded-gate escalation (R-4) appropriately framed, or
  should C1 pre-commit to landing the fix?
- **Q6**: anything missing from out-of-scope, especially around the
  shared const-translation surface with B1/C2?
