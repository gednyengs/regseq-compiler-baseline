# Session Handoff — Bridge Coverage Runner Workstream

Persistent snapshot of in-flight work state for fresh-session
resumption. Designed to be incrementally updated after each
task close so the next session can pick up at full quality.

## Update protocol

- Update **§ Current state** after every task close (new commit SHA,
  in-flight task, escalation queue delta).
- Update **§ Phase arc** only when a new bridge-coverage phase
  completes.
- Update **§ Workflow / Anchors / Tooling / Conventions** only on
  architectural changes (rare).
- **§ Reading order** + **§ First-action recommendations** are
  intentionally stable; touch only if the cold-start flow changes.

The volatility split means 90% of updates touch only the
**Current state** section.

---

## § Current state (UPDATE EVERY TASK CLOSE)

- **Last updated**: 2026-05-24
- **Last completed task**: **regseq-driver-74 (R2: instantiate orphaned mono-collected source functions — tuple `.clone()` `ItemMissingInstance`)** — implementer commit `666ca26`, **assessed APPROVED + Codex soundness pass SOUND**. R2 was DOUBLY mis-scoped (orphan is the field-type's *library* Clone impl, not the tuple CloneShim; and materialising the CloneShim body is copy-only/insufficient) — the filed Option-B-ii fix instantiates source bucket-1 zero-instance Function items via the reactive helper. Mode A **144/144/0**, FF-016 covered. Prior real-fix: R1/driver-73 (FF-004 `ReifyShim`, Mode A was 143).
- **Last commit on main**: `80917ab` — `regseq-driver-75: file R3 spec (constant args-tuple closure call ICE)` (**pushed; local in sync with origin**; a `chore:` HANDOFF-refresh commit sits on top). Recent history: `25542e1` (HANDOFF refresh) → `666ca26` (R2 implementer, last substantive code commit) → `9e55338` (HANDOFF refresh) → `80917ab` (R3/driver-75 spec filed — no code yet, implementer picks up). Codex is invoked via the `codex-runner` Skill (verified working 2026-05-23/24); the local `scripts/python/codex_runner.py` is retired — see §Workflow → Codex invocation.
- **In-flight task**: **regseq-driver-75 (R3) FILED 2026-05-24** (commit `80917ab`, status `pending`; implementer picks up next). **R3 was mis-scoped by the B1 audit (5th this session); the filed spec corrects it.** The B1 audit framed R3 as a "used cloned closure / CloneShim" `fn_sig()`-on-a-closure ICE; a probe disproved both — a plain `let c = move || x; c()` (no clone) ICEs identically, and the path is the closure-**call** rewrite, not `Clone`. **Real bug:** a closure `Fn*::call` whose monomorphised args-tuple arrives as an `Operand::Constant` (zero-arg `const ()`, OR non-empty **all-ZST** args like `c(())` — Codex round 1 widened the breadth from my too-narrow "zero-arg only") makes `rewrite_closure_call_shim` bail (`terminator.rs:286 _ => Ok(None)`), falling through to `translate_call_callee` → `callsite_fn_sig` → `tcx.fn_sig(closure)` → ICE. **Filed fix (Option A):** handle the constant args-tuple in the rewrite (empty → zero positional args; non-empty all-ZST → materialise into a synthetic local + project fields as `Operand::Move`) and replace the silent bail with a typed `DslRejectionKind::Terminator`, so no closure-call shape can reach `tcx.fn_sig(closure)` again. Confined to the rewrite's args-untupling + `FF-017` row + tests; empty case empirically proven, materialise feasibility Codex-verified. Plan (`.harness/audits/2026-05-24-r3-zero-arg-closure-call-plan.md`) + spec (`75-zero-arg-closure-call.md`) each Codex-reviewed 2 rounds. **After R3: F70 residuals** (FF-005 RustCall ABI at `bridge/fnsig.rs:107-115`, RV-009-G slice at `bridge/body/rvalue.rs:197`), then **Category 4** (Section 7 validator audit). Discipline for ALL: HEAD-grounded feasibility probe *before* a plan (it has now caught 5 mis-scopes this session: C1/C2/B1, R2, R3 — and Codex routinely catches a hidden blocker in the first fix draft); full plan→Codex→spec→Codex→file→implementer flow + Type-A assessment, AND **a focused Codex soundness pass on any load-bearing validator/invariant change** ([[assessment-soundness-pass]]). **Quality-Gate gotcha: `cargo clippy --all-targets` has PRE-EXISTING test-target lint drift across BOTH regseq-model AND regseq-driver — don't over-specify `--all-targets` in specs (driver-75's gate scopes clippy to lib + the new test); the real bar is the implementer's own code being clean.** See [[clippy-all-targets-test-drift]].
- **Runner state**: **144 committed sidecar rows**, all PASS via Mode A (Total=144, Pass=144, Fail=0). FF-016 (orphaned field-clone `Item` instance) covered 2026-05-24 by R2/driver-74. FF-004 (`ReifyShim`) covered 2026-05-23 by R1. FF-012 (`CloneShim`) + FF-013 (`FnPtrAddrShim`) covered earlier via zero-bridge-change snippets (body-absent shim flavors, `materialise.rs:246-269`). **R3/driver-75 (filed, not yet implemented) will add `FF-017`** (zero-arg closure call) → 145. F72 added **2 InspectionPredicate arms** (`SyntheticItemKindSub { kind, sub }`; `SignatureNonRigid`).
- **KNOWN DRIFT (open; remedy deferred)**: the matrix per-row `status` column understates runner coverage — at HEAD the sidecar covers 143 rows but the matrix marks only 22 of them `covered` (121 still read `uncovered`/`partial`); separately ~94 matrix-`covered` rows are legacy-fixture-only (not in sidecar). Root cause: the bulk F4 corpus committed sidecar rows without backfilling matrix `status`. The matrix `status` is therefore NOT a reliable coverage view. Read-only report at `scripts/python/coverage_drift.py`; full finding at `.harness/audits/2026-05-23-matrix-sidecar-drift.md`. Remedy = derive matrix status from sidecar under Category 5 (do NOT hand-backfill). A per-gap living ledger was sketched and DEFERRED to Category 5 for the same "derive, don't hand-maintain" reason (`.harness/audits/2026-05-23-gap-ledger-proposal.md`).
- **Active escalation queue**: **2 items remaining** (F72 closed FF-007/RJ-009/SK-004/SK-005; **C1** closed-by-reclassification; **C2/FF-013** + **B1/FF-012** retired-as-mis-scoped AND rows authored; **B2/FF-004 + B2/RJ-024** deferred-by-probe; **F1/RJ-032-F** deferred-pending-phase 2026-05-23 — see below). Open: the F70 residuals **FF-005** (RustCall ABI gap at `bridge/fnsig.rs:107-115`) and **RV-009-G slice** (at `bridge/body/rvalue.rs:197`). Real-fix backlog status: **(R1) driver-73 CLOSED** (const-fn-ptr-provenance; FF-004). **(R2) driver-74 CLOSED** (orphaned mono-collected `Item` instantiation; FF-016 — NOT the F72 lazy-allocation pattern the original framing guessed; see update log). **(R3) driver-75 FILED, awaiting implementer** (constant args-tuple closure-call ICE; FF-017 — NOT the "used-closure clone / fn_sig in CloneShim" the B1 audit guessed; see §Current-state In-flight + update log). All three original framings were mis-scoped and corrected probe-first. Cluster taxonomy below; per-row proposal assignment in `.harness/audits/2026-05-21-post-f4-triage.md` Summary table.
- **F1 (IX-014 + RJ-032-F) RESOLVED 2026-05-23 (no code)**: F1 was the LOW watch-list proposal. **IX-014 is already `covered`** (F71 brought it to PASS via the closure-call rewrite to `ClosureCall { call_kind: CallOnce }`, matched by the FLAT `SyntheticItemKind="ClosureCall"` predicate — `rows.toml:1467`; the `SyntheticItemKindSub` predicate route landed separately in F72 for SK-004/SK-005, not IX-014), so F1's IX-014 watch-list pointer (which targeted path (b), IX-004/IX-010) is obsolete. **RJ-032-F (D8)** is transparently blocked: `target_str_arg_bound` (`peel.rs:179-180`) is hard-wired `None` so D8 short-circuits to `Ok(())`, pending the backend target-binding wiring (spec §26.8/26.9). Reclassified `uncovered`→**`blocked-unimplemented-phase`** (new matrix status class added to the legend — realisable in principle but the producing path is stubbed pending a future phase). **Closes the original F68 slate.** Analysis: `.harness/audits/2026-05-23-f1-rj032f-watchlist.md`. Escalation queue 3 → 2.
- **B2 (FF-004 + RJ-024) RESOLVED-by-deferral 2026-05-23 (probe outcome, no bridge fix)**: B2 was the LOW probe-or-defer proposal. HEAD-grounded probe → **FF-004 (ReifyShim)** has no zero-change shape: runtime reify resolves to flavor `Item` (bridge doesn't apply `resolve_for_fn_ptr`'s track_caller→ReifyShim adjustment), and const/static reify DOES surface a ReifyShim but is rejected at `const_.rs:429` (the C2-family const-fn-ptr-provenance gap) → reclassified `blocked-open-bridge-bug`, unblock = R1. **RJ-024 (UnsupportedEnumLayout)** is `blocked-rustc-infeasible`: its three rejection paths (Union/Float tag, niche-out-of-range) need enum layouts rustc's solver doesn't emit from source. Analysis: `.harness/audits/2026-05-23-b2-deferred-shim-probe.md`. Escalation queue 5 → 3.
- **B1 (FF-012) RETIRED by reclassification 2026-05-23, AND row authored**: the F68 audit mis-scoped B1 as an "array-length const handling" fix to materialize `[u32;4].clone()`'s CloneShim. HEAD-grounded feasibility check + Codex review (APPROVED_WITH_MINOR_FIXES) showed arrays are NOT CloneShim-eligible (rustc synthesizes CloneShim only for FnDef/FnPtr/Coroutine/Closure/Tuple — `rustc_ty_utils/src/instance.rs:256-275`); `[u32;4].clone()` uses a const-generic library impl rejected at `translate_generic_args` (`bridge/ty.rs:744-748`) on the `N=4_usize` const arg — the **same by-design const-generic rejection that retired C1** (deferred driver-37), out of scope. **FF-012 is covered today with zero bridge changes** via a fn-pointer `.clone()` snippet → `CloneShim` flavor (body-absent at `materialise.rs:246-269`). Analysis: `.harness/audits/2026-05-23-b1-cloneshim-retirement.md`.
- **C2 (FF-013) RETIRED by reclassification 2026-05-23, AND row authored**: the F68 audit mis-scoped C2 as a const-eval allocation-provenance fix (`const_.rs:526-542`, accept `Function` provenance → "auxiliary scan materializes FnPtrAddrShim"). HEAD-grounded feasibility check + Codex review (APPROVED_WITH_MINOR_FIXES) showed that mechanism is neither necessary nor sufficient: `FnPtrAddrShim` is the instance for `<fnptr as FnPtr>::addr` (`rustc_ty_utils/src/instance.rs:283-292`), and a const-free `PartialEq::eq(&a,&b)` already bridges to `[Item, Item]` with no FnPtrAddrShim — so removing the const rejection can't surface the flavor. **FF-013 is covered today with zero bridge changes** via a direct `.addr()` snippet (bridge handles it body-absent at `materialise.rs:246-269`). Codex caught a mechanism error in the first draft (core's `eq`/`fn_addr_eq` *do* call `.addr()` at `core/src/ptr/mod.rs:2490,2532`, and rustc *does* mono-collect the addr shim for comparisons — the real reason is that the bridge's instance walk is shallower than rustc's mono closure, surfacing the shim only for a direct user-body call). Conclusion held. The const-provenance rejection is a real but **separate** gap (fn pointers in consts/statics). Analysis: `.harness/audits/2026-05-23-c2-fnptraddrshim-retirement.md`.
- **C1 (OP-003-I) RETIRED by reclassification, not implemented**: the F68 audit mis-scoped C1 as an isolated const-eval fix. Codex plan review (+ verification) found the real blocker is the bridge's **by-design rejection of all const-generic args** (`translate_generic_args` at `bridge/ty.rs:743-748`; reverse at `body/materialise.rs:673-684`, coordinated gap with driver-37) PLUS the deliberate exclusion of `Const::EnumValue` from type-level admissibility (scalar-only at `const_value.rs:54-64`). Architect decided enum/non-scalar const-generics are out of scope for `#![no_std]` driver specs; OP-003-I reclassified `uncovered` → `blocked-bridge-rejects-by-design` (cross-ref Section 6 RJ-029). Analysis: `.harness/audits/2026-05-23-c1-enum-const-eval-plan.md`. **Broad const-generic support remains a deferred gap (driver-37), to be taken up deliberately only if const-generics become a driver-spec requirement.**
- **Uncommitted state in working tree**: clean except two untracked dirs left alone by standing rule — `tests/fixtures/playground-pl011/` + `crates/regseq-driver/tests/fixtures/` (experimentation/fixtures — never commit, never modify). (The earlier `CLAUDE.md` edit was committed by the user as `821ec7d`.)

### Active escalation cluster taxonomy (driver-68's input)

| Cluster | Affected rows | Root-cause family |
| ------- | ------------- | ------------------ |
| (a) Bridge type-projection normalization | RV-007-C, RV-009-G, FF-005 | `Pointee::Metadata` + `<fn() as FnOnce>::Output` projections don't normalize through concrete types |
| (b) Shim materialization gaps | FF-004 (ReifyShim), FF-012 (CloneShim), FF-013 (FnPtrAddrShim) | Distinct shim families. **All three RESOLVED 2026-05-23; the original F68 root-cause guesses here were superseded**: FF-013 + FF-012 are coverable today with zero bridge changes (direct `.addr()` / fn-ptr `.clone()`); FF-004 is `blocked-open-bridge-bug` — runtime reify resolves to `Item` (bridge doesn't apply `resolve_for_fn_ptr`), and const reify emits the ReifyShim but the bridge rejects it on fn-ptr provenance (`const_.rs:429`). |
| (c) Const-eval surface gaps | OP-003-I (Const::EnumValue), FF-013 overlap | `mir::Const::eval` over unevaluated const blocks / fn-pointer provenance |
| (d) Helper / build-contract gaps | TM-006-C (helper overflow_checks=false), RJ-006 (cleanup-pass strips MIR statements), RJ-032-F (target-binding wiring not landed) | Infrastructure / wiring; not bridge bugs |
| (e) Bridge-internal invariant rejections | RJ-024 (UnsupportedEnumLayout) | Rejection paths that rustc's layout solver doesn't produce from source |
| (f) OPEN BUG family pointer | IX-014 | Blocked by IX-004/IX-010 (`Fn::call` resolution in polymorphic MIR) |

Cluster → proposal mapping (per the F68 audit doc):
(a) → A1 HIGH; (b) FF-012 → B1 MEDIUM, (b) FF-004 + (e) RJ-024 → B2 LOW;
(c) OP-003-I → C1 MEDIUM, (c) FF-013 → C2 MEDIUM; (d) TM-006-C + RJ-006 → D1 HIGH,
(d) RJ-032-F → F1 LOW; (f) IX-014 → F1 LOW (bundled with RJ-032-F as "deferred pending external work").

### Next actions (in priority order)

1. **Probes phase DONE (F68 slate complete); R1 DONE (driver-73, FF-004 covered + C2 gap closed + Bucket-1 hardened).** Per the agreed sequence, continue the **real-fix backlog**, each preceded by a HEAD-grounded feasibility probe before a plan (probe-first saved three needless bridge tasks — C1/C2/B1): **(R2) tuple `.clone()` Phase-5 `ItemMissingInstance` — NEXT UP** (apply F72's lazy-allocation pattern from `bridge/body/terminator.rs::lookup_or_create_direct_instance`; empirically: `(u32,u32).clone()` → Phase-5 `ItemMissingInstance` for the tuple's Clone glue item); **(R3) used-closure `.clone()` rustc ICE** (`let d = c.clone(); d()` → `fn_sig()` called on a closure type somewhere in the bridge's CloneShim handling — find the wrong call site). Plan → Codex → spec → Codex → file → implementer + Type-A assessment flow per the F72/R1 pattern; **for any load-bearing validator/invariant change, add a focused Codex soundness pass during assessment** (see [[assessment-soundness-pass]] / the R1 update-log entry — green functional tests masked an UNSOUND Bucket-1 relaxation). Later: F70 residuals (FF-005, RV-009-G), Category 4 (Section 7 validator audit).
2. **F70 residuals** (new task candidates beyond the F68 slate): FF-005 RustCall ABI sig-translation gap at `bridge/fnsig.rs:107-115`; RV-009-G slice case at `bridge/body/rvalue.rs:197`.
3. **Alternative direction**: gap-inventory Category 4 (Section 7 validator audit — ~49 unverified `InvariantViolation` invariants; `bridge-coverage-matrix.md` Section 7 vs `regseq-model/src/model.rs:163` enum + `spec.md` §25.9 registry). Largely unexercised today.
4. **Possible matrix precision patches** as a side track (FF-002 / IX-017 diag-log peel shape mis-classification surfaced in F2/F3 expectations).
5. **Category 5 (deferred, but now has two feeders)**: JSON report output, Mode C CI gating, Mode B agent integration — AND the derived matrix-status reconciliation that fixes the KNOWN DRIFT above (replace hand-backfilled matrix `status` with a value derived from the sidecar + legacy-fixture scan). `scripts/python/coverage_drift.py` is the read-only first slice. Per strategy §10, deferred until corpus stabilises.

---

## § Phase arc (UPDATE ON PHASE CLOSE)

Bridge-coverage-runner workstream phasing per `docs/discussions/regseq-model/bridge-coverage-runner-strategy.md` §11.

| Phase | Task | Commit | Deliverable |
| ----- | ---- | ------ | ----------- |
| F0 | driver-63 | `174c7ff` | Typed `BridgeOutcome` enum + `try_bridge_fixture[_path]` in `test-ir-fixtures` |
| F1 | driver-64 | `9f93c2b` (+rpath fix `e54ddf8`) | Runner crate scaffold (7 modules) + 10-row demo + 3 smoke tests + build.rs rpath wiring |
| F2 | driver-65 | `3a01888` | First snippet batch (22 committed) + mini-F3 (3 arms: SyntheticItemKind / ConstVariant / CallDivergent). IX-014 escalated-before-attempt, IX-017 escalated-after-attempt |
| F3 | driver-66 | `e2e77c0` | Dispatch hardening (14 remaining arms) + sub-axis matcher (kind/position/reason/diagnostic). 22/22 dispatch. Unblocks IX-017 via BodyAbsent |
| F4 | driver-67 | `c8c16f3` | Full corpus batch: 91/102 rows committed, 11 escalations. IX-017 re-attempted and PASSES. Total sidecar: 123 rows |
| F5 | driver-68 | `c3456a5` | Post-F4 batch triage: 7 proposals (A1, B1, B2, C1, C2, D1, F1) covering all 12 active escalations; no auto-filing per strategy §10 Mode B. Audit doc at `.harness/audits/2026-05-21-post-f4-triage.md` |
| F6 (D1) | driver-69 | `660bc7f` | Helper `BuildOverrides` surface (`overflow_checks` + allow-listed `z_flags`) + runner schema + 2 new sidecar rows (TM-006-C, RJ-006). Total sidecar: 125 rows. Two-layer allow-list enforcement (loader `deny_unknown_fields` + helper panic). First proposal-derived task to close per F68 Mode B flow |
| F7 (A1) | driver-70 | `4c1332e` | Bridge `AuthorizedProjectionFolder` at `translate_erased_ty` entry (`bridge/ty.rs`) normalizes `<T as Pointee>::Metadata` (slice + sized) and `<F: FnPtr as FnOnce<Args>>::Output` via `tcx.normalize_erasing_regions` on single alias nodes. Lang-item identity match (`LangItem::Metadata` / `LangItem::FnOnceOutput`); dyn explicitly excluded. 2/3 rows landed (RV-007-C + RV-009-G sized); FF-005 escalated (RustCall ABI rejection at `bridge/fnsig.rs:589-590` BEFORE FnPtrShim materialization). Total sidecar: 127 rows |
| F8 (Cat 1) | driver-71 | `7525ac0` | Closure-family reattempt sweep + matrix-doc OPEN BUG prose refresh. 9/13 cohort rows committed (IX-004 / IX-010 / BD-007 / FT-011-014 / SK-003 / IX-014); 4 escalations (RJ-009 Mode 1 + FF-007 Mode 1 with NEW Phase-5 ItemMissingInstance bridge gap discovery + SK-004 / SK-005 Mode 2 design-time predicate-precision). Matrix-doc `:508-575` refreshed: stale OPEN BUG prose replaced; per-row statuses reclassified per B6 policy; new status class `blocked-bridge-itemmissinginstance` introduced via spec-discretion for FF-007. No bridge code changes. Total sidecar: 136 rows |
| F9 (F71 trio) | driver-72 | `9186248` | F71 follow-up trio, all 3 sub-tasks PASS, **zero new escalations** (bounded gate closed cleanly). Sub-task A: `SyntheticItemKindSub { kind, sub }` sibling predicate arm (Shape A.2.ii) → SK-004 (CallMut) + SK-005 (CallOnce). Sub-task B: lazy-allocate (Path B.2.a) a `ClosureOnceShim`-flavored Instance for the orphaned `FnOnce::call_once` Item via `lookup_or_create_direct_instance` in `bridge/body/terminator.rs`, built as `FnAbi::Rust` to avoid F70's RustCall ABI path; Phase-5 bucket-1 invariant preserved → FF-007. Sub-task C: bounded gate confirmed no source shape surfaces `NonRigidTy::Param` (Param sigs are generic → bucket-3 skip); chose C.2.a.i with new `SignatureNonRigid` predicate reading `item_bridge_flags` from the **in-process** model (spec's serde premise was false — runner does not round-trip the model) → RJ-009 as bucket-4 carve-out coverage. Removes FF-007/RJ-009/SK-004/SK-005 from the escalation queue. Total sidecar: 140 rows. Codex APPROVED (plan 3 rounds, spec 3 rounds). Closed the matrix↔sidecar coverage-drift finding + `coverage_drift.py` tool at `dfa5fad` (separate tooling commit) |
| R1 (1st real fix) | driver-73 | `eaccc41` (impl) + `e89e02d` (assessor hardening) | **First real bridge fix of the workstream** (not a coverage-runner phase; entered the full plan→Codex→spec→Codex→file→implementer→assessment flow). Accept `GlobalAlloc::Function` provenance in const translation (`bridge/const_.rs` `translate_scalar` + `allocation_bytes`): materialize the target Function instance via `lookup_or_create_direct_instance`, emit `Const::Reloc`/`Byte::Reloc`, read the ref back via new `RegseqModel::instance_ref_of` (drift-proof). Covers **FF-004** (`ReifyShim` via `#[track_caller]` static reify); closes the C2 const-fn-ptr gap; VTable stays rejected. Implementer relaxed the Phase-5 Bucket-1 ratchet (ReifyShim shares the reified fn's `DefId`); a focused Codex soundness pass found that UNSOUND (duplicate same-key shims slipped through), fixed by the assessor with a global `validate_unique_instance_keys` backstop + `InvariantViolation::DuplicateInstanceKey`. Total sidecar: 143 rows; Mode A 143/143/0. Plan + spec + assessor-fix all Codex-reviewed |

---

## § Project orientation (STABLE)

- **Regseq compiler**: Rust-to-C device driver compiler. User is the designer/architect.
- **Current workstream**: bridge-coverage-runner — a matrix-coverage discovery engine that turns every uncovered row of `docs/discussions/regseq-model/bridge-coverage-matrix.md` into an executable bridge experiment with deterministic PASS/FAIL.
- **Current phase**: 1a (matches recent commits).
- **Auto-memory**: at `~/.claude/projects/-home-nyengele-Documents-sekekama-regseq-compiler-asplos/memory/`. Carries role + binding rules: compiler-theory-first, no-defensive-fallbacks, spec-before-implementation, three DSL mechanisms, Codex review loop, playground-is-experimentation-space. Loaded on startup.

---

## § Workflow conventions (STABLE)

### Task spec authoring loop

1. Diagnose → draft spec at `.harness/tasks/regseq-driver/NN-name.md`
2. Send to Codex via the **`codex-runner` Skill** (bundled runner) with the four-block prompt template
3. Apply REQUIRED FIXES; re-Codex if substantive (most specs take 2-4 rounds)
4. File via `uv run python scripts/python/task.py add-task --id --name --spec --group --depends-on --phase`
5. Commit + push; implementer picks up

### The 8-section task-spec template

(Matches every recent driver-62 through driver-68 spec.)

1. Description
2. Input Context (Design source / Spec anchors / Bridge code anchors / Prior tasks / Test surface)
3. Implementation Notes (Change A / B / C ...)
4. Non-Goals
5. Test Plan (B1, B2, B3, ...)
6. Review (Files to send / Reviewer focus / Adversarial prompt template — four-block XML)
7. Quality Gate
8. Acceptance Criteria (numbered, concrete)

### Implementer assessment loop (after user signals completion)

1. `git fetch origin && git pull --ff-only origin main`
2. Read the implementer's `.harness/expectations/regseq-driver-NN.md`
3. Verify: `cargo build` / `cargo fmt --check` / `cargo clippy -D warnings` / `cargo test`
4. Manual Mode A invocation of the runner if relevant
5. Verify diff scope confined per Quality Gate
6. Trust-but-verify per CLAUDE.md — don't take the implementer's self-report at face value

### Cargo test gotcha

`cargo test --workspace` in parallel mode hits a rustc-driver stderr-interleaving quirk (E0463 std crate not found). Use `RUST_TEST_THREADS=1 cargo test --workspace -- --test-threads=1` OR test individual crates.

### Codex invocation (canonical)

Use the **`codex-runner` Skill** (the single source of truth for invoking
Codex). Its bundled runner is pure stdlib — plain `python3`, no `uv run`.
The Skill supersedes the repo's local `scripts/python/codex_runner.py`
copy (do not invoke the local copy; verified equivalent + working
2026-05-23).

```bash
cat <<'EOF' | python3 "/home/nyengele/.claude/skills/codex-runner/scripts/codex_runner.py" --timeout 900
<task>
... what to review/identify ...
</task>

<grounding_rules>
- Cite file:line for every claim.
- Mark hypotheses as hypotheses.
- Don't speculate about code or spec sections you didn't read.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
<per-check fields with PASS/FAIL + citation>
NEW ISSUES: numbered or "none"
REQUIRED FIXES: numbered (only if VERDICT != APPROVED)
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
EOF
```

Bash `timeout` >= `950000` (Codex `--timeout 900` + margin). Default `--effort high`. Codex empirically probes code (sometimes runs `cargo test`) — trust findings. Useful flags: `--message-only` (raw `final_message`, skip the JSON wrapper), `--no-yolo` (deny unattended shell), `--model` / `--effort` / `--cwd`.

History (2026-05-23): the Codex runner moved to a user-level `codex-runner` Skill (`821ec7d` slimmed CLAUDE.md §2 to an "External Review" pointer). The Skill was verified working end-to-end and is now the canonical entry; the local-script invocation was retired per user decision. The repo still ships `scripts/python/codex_runner.py`, but do not use it — go through the Skill.

If Codex returns `error: codex produced no agent_message`, daemon may be rate-limited; try `codex exec --json -m gpt-5.5` directly.

### Authoring protocol for snippet batches (F2, F4, future)

- Per-row 3-retry budget. Snippets that fail all 3 retries escalate.
- **Escalated rows stay OUT of sidecar entirely** (no `[[row]]`, no snippet file).
- Escalations documented in expectations file with candidate shapes + empirical outcome + follow-up direction.
- **Anti-silent-pass discipline**: `Inspection::Unimplemented => FAIL`. (F3 removed the `Unimplemented` variant after full dispatch.)

### Commit conventions

- Conventional prefixes (`regseq-driver-NN:`, `feat:`, `fix:`).
- **No author attributions** (per global CLAUDE.md). HEREDOC for multi-line bodies.
- Implementer's commit message + my registration commit message differ in tone but share the prefix.

---

## § Key file anchors (STABLE; update on architectural change)

### Strategy + matrix (design sources)

- `docs/discussions/regseq-model/bridge-coverage-runner-strategy.md` — H1 title reads "v2" but body content is v3 per change-log section (acknowledged editorial gap in driver-65/66/67/68 specs).
- `docs/discussions/regseq-model/bridge-coverage-matrix.md` (v2.1)
- `docs/discussions/regseq-model/spec.md` — locked phase-1 spec
- `docs/discussions/regseq-model/pretty-printer.md`

### Runner crate (`crates/bridge-coverage-runner/`)

- `src/inspect.rs` — 22/22 InspectionPredicate variants. `InspectError` carries `NotFound` + `AnyFailed` (F3 removed `Unimplemented`).
- `src/matrix.rs` — TOML loader + subset-OK drift check. `ExpectedRejection` has variant + kind + position + reason + diagnostic.
- `src/snippet.rs` — snippet loader + SHA-256 hash prefix
- `src/oracle.rs` — subprocess rustc pre-flight (`--emit=metadata`) + in-process bridge via `try_bridge_fixture_path` inside `catch_unwind`
- `src/cluster.rs` — fingerprint + grouping
- `src/report.rs` — markdown report emitter
- `src/main.rs` — CLI + `expected_rejection_matches` (signature: `Result<(), String>` NOT bool) with sub-axis precision
- `build.rs` — `cargo:rustc-link-arg=-Wl,-rpath,<sysroot>/lib` for direct binary on macOS
- `matrix_metadata/rows.toml` — 123 committed rows
- `snippets/<row_id>.rs` — 123 snippet files
- `snippets/README.md` — naming convention + `"PointerCoercion::<sub>"` dotted-compound for CastKind args

### F0 helper (read-only after F0)

- `crates/test-ir-fixtures/src/lib.rs:45-72` — `BridgeOutcome` enum (4 variants: Ok / ValidatorFailed / Rejected / RustcRejected)
- `:156` — `try_bridge_fixture(name)`
- `:183` — `try_bridge_fixture_path(path)` (sanitizes hyphenated stems)
- `:282` — `try_compile_and_bridge_with_crate_name` workhorse
- `:322` — `-C panic=abort` injection (universal)
- `:338-344` — rustc config (`no_codegen=true`, `mir_opt_level=0`, `always_encode_mir=true`, `overflow_checks=false`, `debug_assertions=false`). **The `overflow_checks=false` suppresses `BinOp::*WithOverflow` emission for ordinary signed `-`/`*`; use `core::intrinsics::*_with_overflow` directly when targeting those variants.**

### Bridge surface (consumed read-only)

- `crates/regseq-driver/src/bridge/mod.rs:376` — `BridgeError` enum
- `:379-384` DslRejection variant; `:580-597` DslRejectionKind enum
- `:480-487` UnsupportedCAbiType; `:363-371` CAbiTypePosition
- `:513-518` DiagLogFormat; `:535-551` DiagLogFormatReason (6 reasons)
- `:524-529` DiagLogValidation; `:556-575` DiagLogValidationDiagnostic (D1/D2/D4/D6/D7/D8)
- `:908-955` Linkage producer (only Default/NoMangle/Foreign; no Weak)
- `crates/regseq-driver/src/bridge/ty.rs:277-280` Coroutine rejection
- `crates/regseq-driver/src/bridge/ty.rs:1471-1477` substantive-HRTB rejection rule
- `crates/regseq-driver/src/callbacks.rs:103-168` production RegseqCallbacks (mirror of F0 helper config)
- `crates/regseq-driver/build.rs` — production driver's rpath wiring (F1 mirrors this)

### Model surface (consumed read-only)

- `crates/regseq-model/src/{ty,item,instance,synthetic,const_value,statement,terminator,rvalue,place,basic_block,body}.rs` — enums + structs walked by inspect dispatch arms
- `BasicBlock.terminator` is NON-optional per `basic_block.rs:45`

### Test artifacts

- `crates/regseq-driver/tests/bridge_ty.rs:447-577` — HRTB shape declarations + per-variant assertions (canonical reference for RJ-022 / coroutine / dyn shapes)

### Harness

- `.harness/README.md` — conceptual entry point
- `.harness/SESSION.md` — worker bootstrap doc
- `.harness/HANDOFF.md` — **this file**, session-state snapshot
- `.harness/tasks/regseq-driver/NN-*.md` — task specs
- `.harness/expectations/regseq-driver-NN.md` — implementer reports
- `.harness/audits/YYYY-MM-DD-<topic>.md` — date-prefixed audit docs
- `scripts/python/task.py` — task CLI
- Codex review: **`codex-runner` Skill** → `/home/nyengele/.claude/skills/codex-runner/scripts/codex_runner.py` (canonical; the repo's `scripts/python/codex_runner.py` copy is retired — do not use)

---

## § Tooling specifics (STABLE)

### Direct binary invocation (post-rpath-fix)

```bash
target/debug/bridge-coverage-runner \
  --metadata crates/bridge-coverage-runner/matrix_metadata/rows.toml \
  --snippets crates/bridge-coverage-runner/snippets/ \
  --matrix-md docs/discussions/regseq-model/bridge-coverage-matrix.md \
  --out /tmp/brcr-out/
```

Expects exit 0, `Total = Pass`, `Fail = 0`.

### task.py add-task

```bash
uv run python scripts/python/task.py add-task \
  --id regseq-driver-NN \
  --name "<title>" \
  --spec .harness/tasks/regseq-driver/NN-<slug>.md \
  --group regseq-driver \
  --depends-on regseq-driver-PREV \
  --phase 1a
```

### Toolchain

- Pinned `nightly-2026-02-01` per `rust-toolchain.toml`
- Workspace member registration explicit in `Cargo.toml:3-14` (alphabetical)

---

## § Constraints + gotchas (STABLE)

- **No defensive fallbacks** (memory `feedback_no_defensive_fallbacks`). Toolchain is pinned; don't add shadow fallbacks for hypothetical rustc regressions.
- **No author attributions** in commits (global CLAUDE.md).
- **Playground is experimentation space**. Never modify `tests/fixtures/playground/`. Don't cite as load-bearing in specs. Don't include in commits.
- **F0 surface is read-only** after F0 close. F1/F2/F3/F4 architecture diffs vs main should be empty for the listed module files when implementing new tasks.
- **No new dependencies** in the runner crate beyond F1's set (`test-ir-fixtures`, `regseq-model`, `regseq-driver`, `toml`, `serde + derive`, `sha2`).
- **CastKind args use dotted-compound** for `PointerCoercion::<sub>`. Documented in `snippets/README.md`.
- **Sub-axis matcher field semantics**: `Option::None => no constraint`; precise matching when field is `Some(...)`.
- **Direct binary on macOS** requires `build.rs` rpath wiring (shipped F1 followup).
- **Authoring**: per-row 3-retry budget; escalations stay OUT of sidecar; documented in expectations.
- **Strategy §10 Mode B**: agents "propose decomposition", don't auto-file. Human approval gates `task.py add-task` for proposal-derived tasks.

---

## § Reading order for cold start (STABLE)

1. **This handoff document** (you're reading it)
2. Auto-memory (loads automatically)
3. `git log --oneline -10` — see the F0-F5 progression
4. The newest task spec at `.harness/tasks/regseq-driver/` (currently `68-post-f4-batch-triage.md`)
5. The newest expectations file at `.harness/expectations/` (currently `regseq-driver-67.md` — F4 implementer report with full escalation log)
6. `docs/discussions/regseq-model/bridge-coverage-runner-strategy.md` §10 + §11 — strategy phasing + Mode B
7. `.harness/README.md` — harness conceptual model (only if doing new spec authoring)
8. Skip detailed reads of F1/F2/F3/F4 specs unless user asks about a specific historical decision

---

## § First-action recommendations by user input (STABLE)

**User says "implementer is done"**:

Common pre-pull steps (both task types):
1. `git fetch origin && git status` — note current local HEAD; this is the **diff base** (the spec-registration commit). The implementer's commit becomes the new HEAD after pull.
2. `git log --oneline HEAD..origin/main` — confirm exactly one new commit.
3. `git pull --ff-only origin main`
4. `git show --stat <implementer-sha>` — orient on what files moved.
5. Read `.harness/expectations/regseq-driver-NN.md` end-to-end.

Then branch by task type:

**(A) Code-modifying task** (driver-63 through -67, future bridge/runner edits):

6a. `cargo build -p <crate>` clean
7a. `cargo fmt --all -- --check` clean
8a. `cargo clippy -p <crate> --all-targets -- -D warnings` clean
9a. `cargo test -p <crate> -- --test-threads=1` green (or workspace with `RUST_TEST_THREADS=1`)
10a. Manual Mode A invocation of the runner if runner-touching task: confirm `Total = Pass`, `Fail = 0`, exit 0
11a. Diff scope verification: `git diff --stat <spec-reg-sha>..<implementer-sha>` confined per Quality Gate. **Read the implementer's commit carefully** — they may have added defensible adaptations outside the strict spec scope (e.g., F3 implementer touched `tests/smoke.rs` to make a monotonic assertion + removed `InspectError::Unimplemented` variant as orphan cleanup). Judgment call: defensible engineering hygiene vs. scope creep. The litmus test: does the change trace to a consequence of completing the spec's mandate, or is it independent improvement?
12a. **Spot-check empirical claims** from the expectations file. If the implementer reports "Mode A produced Total=N Pass=N Fail=0", re-run Mode A and confirm. If they report a probe outcome (e.g., F3's IX-017 BodyAbsent probe = YES), re-run the probe shape. Trust-but-verify.

**(B) Analytical task** (driver-68 post-F4 triage, future audit/triage tasks):

6b. `cargo test --workspace -- --test-threads=1` green (sanity; no code changes expected)
7b. Diff scope verification: `git diff --stat <spec-reg-sha>..<implementer-sha>` should touch ONLY the deliverable document(s) + harness state + tasks.json registration. **No source modifications, no `task.py add-task` calls beyond the F68 self-registration.**
8b. Read the deliverable document end-to-end (e.g., `.harness/audits/<date>-<topic>.md` for triage tasks).
9b. Coverage verification: every input item (e.g., every active escalation for a triage task) appears somewhere in the deliverable.
10b. Field completeness verification per the spec's Acceptance Criteria (e.g., for triage proposals: title, affected escalations, root cause, empirical evidence, scope sketch, priority, dependencies, effort).
11b. Coherence check: does the cluster/grouping/recommendation taxonomy match the empirical evidence? Spot-check by reading 2-3 source-citations to confirm they say what the deliverable claims they say.
12b. **No-auto-filing verification** (for triage tasks): `git diff <spec-reg-sha>..<implementer-sha> -- .harness/tasks.json` should show ONLY the task's own status flip (`pending` → `done`), no other add-task entries.

Common post-verification:
13. **Update §Current state of this handoff**. Four specific edits:
    - `Last updated`: today's date
    - `Last completed task`: the task that just closed
    - `Last commit on main`: the implementer's SHA + commit subject (truncated)
    - `In-flight task`: typically "none" until the next is filed
    - **Active escalation queue**: refresh count + cluster taxonomy delta if the closed task resolved any escalations (e.g., F4 resolved IX-017 via BodyAbsent re-attempt)
    - **Next actions**: rewrite the priority list for what comes next
14. Append a `YYYY-MM-DD: <one-line summary>` entry to **§ Update log**.
15. Commit + push the handoff update in the same session that did the assessment.
16. Summarize implementer outcome to user. **A good summary covers**:
    - Verification result table (build / fmt / clippy / tests / Mode A — pass/fail per check)
    - Scope of changes (file count, line count, diff stat)
    - Surprising findings the implementer surfaced beyond the spec's predictions (these go into the next triage queue)
    - Scope-discipline judgment calls (defensible adaptations vs. potential creep — flag both)
    - Recommended next direction (file next task / wait for user / pause for review)

**User wants to file a new task**:
1. Draft spec via the 8-section template
2. Send to Codex (2-4 rounds typical)
3. Apply REQUIRED FIXES
4. File via `task.py add-task`
5. Commit + push
6. Update **§ Current state**

**User wants to start something outside bridge-coverage-runner**:
- This handoff covers the bridge-coverage track specifically. Ask which direction; capture that workstream in a separate state file or extend this one with a `§ Other workstreams` section.

**User wants to compact / hand off again**:
- Update **§ Current state** to reflect latest commit + escalations
- The rest of the file is stable; no other updates needed

---

## § Update log (append-only)

- 2026-05-21: Initial handoff document created. State as of commit `705e9a6` (driver-68 filed).
- 2026-05-21: F68 assessment closed. Implementer commit `c3456a5` verified clean (3 files: triage doc, expectations, tasks.json status flip; no code changes; no auto-filing). All 12 escalations covered by 7 proposals. Workspace `cargo test` green (79 test groups, 0 failures). Awaiting user approval of subset for driver-69+ spec drafting.
- 2026-05-21: Codex review pass over the F68 triage deliverable returned NOT_READY with 5 REQUIRED FIXES. Audit doc revised in a follow-up commit per user direction: (#1) F1 IX-014 reframed as F68 spec-policy choice with explicit deferred path-(a) note from F2 expectations `:238-243`; (#2) B2 root cause downgraded from "empirically-confirmed no source path" to "no source path within F4's 3-retry budget"; (#3) B1 carries an explicit fix-location warning pointing at `ty.rs:614-618` (`translate_generic_args`) as the likely actual blocker, with mandate that the B1 spec-drafting session must empirically pin the location; (#4) Closing recommendations now flags B1 / C1 / C2 const-surface coordination risk; (#5) MEDIUM-tier ordering rationale formalized. Defects #1, #2, #4, #5 fixed in-doc; defect #3 deferred to B1 spec-drafting session per user direction.
- 2026-05-21: regseq-driver-69 (D1) drafted + Codex-reviewed (2 rounds: NOT_READY → APPROVED_WITH_MINOR_FIXES, all fixes applied) + filed + pushed at commit `32dad48`. Spec extends test-ir-fixtures with a per-fixture `BuildOverrides { overflow_checks, z_flags }` surface (allow-listed at both matrix loader load-time and helper workhorse defense-in-depth), threads it through the runner schema, and authors TM-006-C + RJ-006 sidecar rows. Empirical claims grounded in pinned-nightly rustc source (Codex verified via active sysroot at `~/.rustup/toolchains/nightly-2026-02-01-aarch64-apple-darwin/lib/rustlib/rustc-src/rust/compiler/`). Three local commits (`11124b3`, `4e14bb1`, `32dad48`) pushed to origin/main in a single push. Implementer agent picks up next.
- 2026-05-21: regseq-driver-69 closed by implementer at commit `660bc7f`. Assessment loop (Type A code-modifying) verified clean: `cargo build` green, `cargo fmt --all -- --check` clean, `cargo clippy -p test-ir-fixtures -p bridge-coverage-runner -- -D warnings` clean, `cargo test` green (7 test groups, 18 unit tests, 0 failures), Mode A invocation produced `Total=125 Pass=125 Fail=0` exit 0. TM-006-C reports PASS (positive, Ok, inspection passed against AssertKind::OverflowNeg). RJ-006 reports PASS (rejected, DslRejection { kind: Statement } matched). Diff scope confined to 8 expected files (test-ir-fixtures src/lib.rs + tests/overrides.rs; bridge-coverage-runner src/{matrix,oracle,main}.rs + matrix_metadata/rows.toml + 2 snippets) plus tasks.json status flip. Acceptance Criteria all green; the implementer did NOT ship `.harness/expectations/regseq-driver-69.md`, which is a deviation from project convention but not a Quality Gate violation (the D1 spec did not list it as an Acceptance Criterion).
- 2026-05-22: regseq-driver-70 (A1: bridge associated-type projection normalization) filed + pushed at commit `861177f`. 2 Codex rounds; APPROVED_WITH_MINOR_FIXES. Authorized normalization restricted to slice + sized `Pointee::Metadata` and `Fn`-family `::Output`; dyn-Pointee::Metadata explicitly out of scope due to DynMetadata Foreign hazard (separate task class).
- 2026-05-22: gap inventory beyond F68 produced at `.harness/audits/2026-05-22-gap-inventory.md`. Four Codex rounds (NOT_READY → NOT_READY → NOT_READY → APPROVED_WITH_MINOR_FIXES); each round caught real defects. Material findings: (a) the OPEN BUG IX-004/IX-010 prescan bug claimed by the matrix at `:508-523` was already fixed by regseq-driver-62 — the matrix prose is stale; (b) matrix has 359 rows (299 non-validator + 60 validator), not the stale "~445" figure in earlier HANDOFFs; (c) F69-close drift count of 234 missing decomposes as 96 covered-via-aliasing + 20 partial + 60 validator + 15 blocked + 43 actually uncovered; (d) of those 43, 27 remain after excluding F68 A1-F1 targets and Category-1 closure cohort; (e) FF-002 `covered` matrix claim is likely a false-positive. Six gap categories enumerated: closure-family reattempt sweep (13 rows), FF-002 audit, F7 over 27 rows, Section 7 validator audit (49 unknown), F5 automation slate, matrix-precision pass + small runner/bridge triggers. None auto-filed.
- 2026-05-23: regseq-driver-70 (A1) closed by implementer at commit `4c1332e`. Assessment loop verified clean: build / fmt / clippy / 5 B2 tests all green; Mode A reports Total=127 Pass=127 Fail=0 (sidecar 125 → 127). 2/3 rows landed (RV-007-C + RV-009-G sized); **FF-005 ESCALATED** with empirically grounded rationale (verified via `bridge/fnsig.rs:589-590`: `translate_abi(ExternAbi::RustCall)` rejects with UnsupportedAbi BEFORE FnPtrShim materialization). Implementer respected the spec's "NO bridge changes outside ty.rs" Non-Goal and used F4-style escalation discipline (excluded from sidecar, inline rationale in rows.toml). RV-009-G slice case also surfaced a separate pre-existing bridge gap at `bridge/body/rvalue.rs:197`. Both become new follow-up task candidates beyond the F68 audit's existing slate; gap inventory at `.harness/audits/2026-05-22-gap-inventory.md` may benefit from a future addendum noting these post-A1 discoveries. Assessment verdict: APPROVED-WITH-DEVIATION; 2 of 17 ACs strictly fail (AC 7 FF-005 PASS, AC 8 Total=128) but the escalation is well-justified and net forward progress is real.
- 2026-05-23: regseq-driver-71 (Category 1) drafted + Codex-reviewed (5 rounds: NOT_READY x4 → APPROVED) + filed + pushed at commit `5759691`. 13-row cohort with three-tier outcome model: Tier 1 (9 likely PASS), Tier 2 (2 uncertain may hit RustCall ABI), Tier 3 (2 design-time escalation due to missing SyntheticItemKind sub-axis predicate). Each Codex round caught real defects: SK predicate-precision gap; Tier-1 count text; 4 citation drifts; matrix-refresh prose; escalation-mode split (Mode 1 empirical / Mode 2 design-time); AC mode split; widened reclassification taxonomy for mixed-baseline rows; one drifted gap-inventory citation. Final APPROVED verdict has no NEW_ISSUES and no REQUIRED_FIXES.
- 2026-05-23: regseq-driver-71 closed by implementer at commit `7525ac0`. Assessment loop verified clean: fmt clean / cargo test workspace green (81 test groups, 0 failures) / Mode A reports Total=136 Pass=136 Fail=0 (sidecar 127 → 136). 9 of 13 cohort rows committed (IX-004 / IX-010 / BD-007 / FT-011-014 / SK-003 / IX-014); 4 escalations (RJ-009 Mode 1 — couldn't surface NonRigidTy{Param} against HEAD's bucket-4 path; FF-007 Mode 1 — surfaced NEW Phase-5 ItemMissingInstance bridge gap at ClosureOnceShim materialization, distinct from F70's RustCall ABI; SK-004 + SK-005 Mode 2 design-time per spec). Implementer used AC-13 spec discretion to introduce new matrix status class `blocked-bridge-itemmissinginstance` for FF-007. IX-014 PASSed via the closure-call rewrite to `ClosureCall { call_kind: CallOnce }`; coherent layered documentation notes that FF-007 sub-axis stays uncovered (separate bridge gap) and SK-005 sub-axis stays masked (predicate-precision gap). Net forward progress: sidecar +9 rows, 7% expansion. Three new follow-up task candidates surfaced beyond F68: (a) FF-007 ClosureOnceShim Phase-5 ItemMissingInstance bridge gap; (b) RJ-009 source-shape discovery; (c) SyntheticItemKind sub-variant predicate (unblocks SK-004/005). Assessment verdict: APPROVED.
- 2026-05-23: regseq-driver-72 (F71 follow-up trio) drafted via plan-first flow — plan doc (`.harness/audits/2026-05-23-f71-followup-trio-plan.md`) Codex-reviewed 3 rounds (NOT_READY → APPROVED_WITH_MINOR_FIXES → APPROVED), then full spec Codex-reviewed 3 rounds (APPROVED_WITH_MINOR_FIXES x2 → APPROVED) — filed + pushed at `43a8a26`. Bundled the three F71-surfaced gaps: sub-task A (SyntheticItemKind sub-variant predicate, A.2.ii recommended), B (ClosureOnceShim Phase-5 binding, paths a/b), C (RJ-009 source-shape with bounded gate, C.2.a split into α/β serde sub-paths). Codex caught real defects each round: A.2.i serde backward-compat hazard; C.2.a runner-outcome-model underspecification; wrong spec.md §9.7 citation; missing Phase-5-weakening exclusion; F70 AuthorizedProjectionFolder coordination; the serde-skip location (item_bridge_flags vs signature_non_rigid); a nonexistent F70 expectation-file reference; multi-suffix-id and undeclared-status edge cases in the drift tool.
- 2026-05-23: regseq-driver-72 closed by implementer at commit `9186248`. Assessment loop verified clean: Mode A independently re-run → Total=140 Pass=140 Fail=0 (sidecar 136 → 140); all 4 cohort rows (SK-004 / SK-005 / FF-007 / RJ-009) PASS with matrix status backfilled to `covered`. All 3 sub-tasks PASS on recommended paths, **zero new escalations** (first task in the arc not to spawn a new gap — sub-task C's bounded gate closed cleanly with code-level proof that no `Param` rejection can surface). Notable correct deviation: sub-task C found the spec's serde premise FALSE (the runner inspects the in-process `RegseqModel`, not a deserialized one — verified at `test-ir-fixtures/src/lib.rs:470` + `oracle.rs:99`), so `SignatureNonRigid` reads `item_bridge_flags` directly with NO serde/model/item edits (avoided dead code per CLAUDE.md §1.2). Sub-task B built the shim sig as `FnAbi::Rust` to stay clear of F70's RustCall ABI gap. rows.toml churn = +4 rows minus now-obsolete F71 escalation comment blocks (legitimate, not scope creep). Assessment verdict: APPROVED (clean — all ACs satisfied). Separately surfaced + documented the matrix↔sidecar coverage drift (matrix `status` understates runner coverage by 121 rows; root cause = bulk F4 corpus didn't backfill matrix status) with read-only tool `scripts/python/coverage_drift.py` (Codex-reviewed) at `dfa5fad`; remedy (derive matrix status from sidecar) deferred to Category 5; per-gap living-ledger sketch also deferred to Category 5 (same derive-don't-hand-maintain reason).
- 2026-05-23: regseq-driver-73 (R1) CLOSED by implementer (`eaccc41`) + **assessor hardening**. Type-A assessment: build ✅, fmt ✅, `regseq-model` lib 901 ✅, new model tests ✅, new bridge test `bridge_const_fnptr_provenance` 6/6 ✅, Mode A **143/143/0** with FF-004 (`ReifyShim`) PASS. clippy `-p regseq-model --all-targets -D warnings` fails with **23 PRE-EXISTING** errors (verified at base `73cf135`; clippy-version drift in untouched test files `pretty.rs`/`layout.rs`/etc. — prior Quality Gates never linted regseq-model tests; NOT a driver-73 regression — repo-hygiene follow-up candidate). Implementation faithful to spec (const `GlobalAlloc::Function` arm at both sites, F72-style materialization via `lookup_or_create_direct_instance`, drift-proof `instance_ref_of` read-back, VTable still rejected). **Notable deviation, beyond-spec but necessary:** the implementer relaxed the Phase-5 Bucket-1 completeness ratchet because `ReifyShim(t)` shares the reified fn's `DefId` (item `t` carries Item + ReifyShim). A focused Codex soundness pass found that relaxation **UNSOUND** — bounding only the `Item`-flavored count let duplicate same-key shims (`Item + ReifyShim + ReifyShim`) pass, and `validate_f4_instance_order` only rejects *descending* keys, not *equal* ones (`model.rs:3336`), so nothing caught the dup; the ratchet had come to rely on producer-side dedup. **Assessor fix** (separate commit): added `InvariantViolation::DuplicateInstanceKey` + a global `validate_unique_instance_keys` (HashMap over all instances) called from BOTH `validate()` and `validate_bridge_completeness`, plus 2 negative/positive bucket tests. Re-Codex → **SOUND** (no false positives: FF-004 Item+ReifyShim, F72 ClosureOnceShim-only, generic Bucket-3 multi-mono, and the 143-row corpus all verified; distinct flavors → distinct keys). Lesson recorded: **load-bearing validator/invariant changes need a focused Codex soundness pass during assessment** — the functional verification (all tests green, Mode A 143) would have shipped the latent hole otherwise. R1 is the first real-fix-backlog item closed; R2/R3 remain.
- 2026-05-23: regseq-driver-73 (R1: accept `GlobalAlloc::Function` provenance in const translation) drafted via the plan-first flow + filed. Plan (`.harness/audits/2026-05-23-r1-const-fnptr-provenance-plan.md`) Codex-reviewed → APPROVED_WITH_MINOR_FIXES (hidden-blocker check CLEARED: rank-0 Function `Const::Reloc`/`Byte::Reloc` already validate via `invariant_9_pass_const_reloc_and_bytes_reloc` — so R1's fix-hook is correct, the first const proposal this session NOT mis-scoped after C1/C2/B1); 4 fixes applied (terminator path, "cannot compute"→"avoid drift", hidden-blocker evidence, R-2 bounded + regression coverage). Spec (`.harness/tasks/regseq-driver/73-const-fnptr-provenance.md`, 8-section template) Codex-reviewed → APPROVED_WITH_MINOR_FIXES; 4 fixes applied (wrong `check_instance_ref` anchor → `model.rs:3381-3395`; `spec.md`→full path; B5 aggregate-vtable shape so it routes through `allocation_bytes`; exact post-mono fn-sig recipe `tcx.fn_sig(def_id).instantiate(...)`→`translate_fn_sig`). Codex confirmed `function_instance_flavor_for_kind` is `pub(crate)`, `translate_fn_sig` public, 143-row count, and Phase-5 static/const ordering safe. The fix: 4 changes (A: shared Function-provenance helper materializing the instance via `lookup_or_create_direct_instance`; B: wire both const sites, keep VTable rejected; C: `pub(crate)` lookup_or_create; D: model `instance_ref_of(id)` API to drift-proof the InstanceRef). First code-writing task of the session — implementer picks up; assessment loop on completion (expect Mode A 143 rows, FF-004 covered). FIRST real-fix backlog item entering the implementer flow.
- 2026-05-23: F1 (IX-014 + RJ-032-F watch-list, F68 LOW) RESOLVED — NO code. **Closes the original F68 proposal slate** (A1, B1, B2, C1, C2, D1, F1). IX-014 is already `covered` (F71 brought it to PASS via the closure-call rewrite to `ClosureCall { call_kind: CallOnce }`, matched by the FLAT `SyntheticItemKind` predicate at `rows.toml:1467`; the `SyntheticItemKindSub` predicate landed separately in F72 for SK-004/SK-005, NOT for IX-014), so F1's IX-014 watch-list pointer (path (b)) is obsolete — no action. RJ-032-F (D8) is transparently blocked: `target_str_arg_bound` (`peel.rs:179-180`) is hard-wired `None` so the D8 check short-circuits to `Ok(())`, pending the backend target-binding wiring (spec §26.8/26.9) — not a bug, not a rejection, not rustc-infeasible. Introduced a new matrix status class **`blocked-unimplemented-phase`** (realisable in principle, producing path deliberately stubbed pending a future phase; distinct from blocked-open-bridge-bug which panics/rejects, and blocked-spec-gap which has no realisable witness) and reclassified RJ-032-F `uncovered`→`blocked-unimplemented-phase`. Analysis: `.harness/audits/2026-05-23-f1-rj032f-watchlist.md`. Escalation queue 3 → 2. Session probe-first tally: F68 slate closed = 3 mis-scoped retires (C1/C2/B1) + 2 deferrals (B2/F1), 2 rows covered with zero bridge changes (FF-012/FF-013), 4 real gaps surfaced (R1 const-fn-ptr-provenance, R2 tuple-clone ItemMissingInstance, R3 closure-clone ICE; FF-004 folds into R1).
- 2026-05-23: B2 (FF-004 ReifyShim + RJ-024 UnsupportedEnumLayout, F68 LOW probe-or-defer) RESOLVED by DEFERRAL — NO code, NO bridge fix. HEAD-grounded probe (per the agreed probes-first sequence): FF-004 has no zero-change shape — runtime `#[track_caller]` reify resolves the cast operand to flavor `Item` (the bridge does not route reify casts through `resolve_for_fn_ptr`, so no track_caller→ReifyShim adjustment), and const/static reify (`static FP: fn() = tc_fn`) DOES surface a ReifyShim but is rejected at `const_.rs:429` (`pointer scalar over Function`) — the C2-family const-fn-ptr-provenance gap. Reclassified `uncovered`→`blocked-open-bridge-bug`; unblock = the R1 const-provenance fix (`const_.rs:429`+`:540`), which now motivates FF-004 + the C2 gap. RJ-024's three rejection paths (Union/Float tag, niche-out-of-range — `ty.rs:1320/1368/1446`) need enum layouts rustc's solver doesn't emit from source (probed niche / `repr(u128)` / niche-squeeze, none hit it); reclassified `uncovered`→`blocked-rustc-infeasible`. B2 removed from the active queue (5→3). Analysis: `.harness/audits/2026-05-23-b2-deferred-shim-probe.md`. Both Codex passes this session keep paying off; the probe-first discipline now: 3 mis-scoped retires + 1 clean defer, 2 rows covered with zero bridge changes, 4 separate gaps surfaced.
- 2026-05-23: B1 (FF-012, F68 audit proposal) RETIRED by reclassification — NO bridge code — AND its target row FF-012 authored as a passing sidecar row (sidecar 141 → 142, Mode A 142/142/0). Third consecutive F68 proposal found mis-scoped by a HEAD-grounded feasibility probe (after C1, C2). The audit proposed fixing "array-length const handling" for `[u32;4].clone()`, but rustc synthesizes `CloneShim` only for FnDef/FnPtr/Coroutine/Closure/Tuple self types (`rustc_ty_utils/src/instance.rs:256-275`) — ARRAYS are excluded and use a const-generic library Clone impl, so `[u32;4].clone()` is rejected at `translate_generic_args` (`bridge/ty.rs:744-748`) on the `N=4_usize` const arg = the SAME by-design const-generic rejection that retired C1 (deferred driver-37), out of scope. FF-012 is coverable today with ZERO bridge changes via a fn-pointer `.clone()` snippet → `CloneShim` flavor (body-absent at `materialise.rs:246-269`; the "derived Clone impl" matrix description was a red herring — a derive makes a user impl, flavor Item). Codex review (APPROVED_WITH_MINOR_FIXES) confirmed the conclusion; one correction: the closure-clone ICE (`fn_sig()` on a closure) reproduces only when the cloned closure result is USED (`let d = c.clone(); d()`) — a bare unused `c.clone();` hits `ItemMissingInstance` instead. Two SEPARATE gaps logged (NOT B1/FF-012): tuple `.clone()` Phase-5 `ItemMissingInstance` (FF-007 family); used-closure-clone ICE (ICE-class bridge bug). Analysis: `.harness/audits/2026-05-23-b1-cloneshim-retirement.md`. Escalation queue 6 → 5.
- 2026-05-23: C2 (FF-013, F68 audit proposal) RETIRED by reclassification — NO bridge code — AND its target row FF-013 authored as a passing sidecar row (sidecar 140 → 141, Mode A 141/141/0). HEAD-grounded feasibility sanity-check (per the C1/F72 lesson) found C2's const-fix premise false: `FnPtrAddrShim` = `<fnptr as FnPtr>::addr` (`rustc_ty_utils/src/instance.rs:283-292`), and a const-free `PartialEq::eq(&a,&b)` already bridges to `[Item, Item]` with no FnPtrAddrShim — so removing the `const_.rs:526-542` `GlobalAlloc::Function` rejection cannot surface the flavor. FF-013 is coverable today with ZERO bridge changes via a direct `.addr()` snippet (`#![feature(fn_ptr_trait)]`; bridge handles it body-absent at `materialise.rs:246-269`). Codex review (APPROVED_WITH_MINOR_FIXES) confirmed the conclusion but caught a mechanism error in the first draft: core's `eq`/`fn_addr_eq` DO call `.addr()` (`core/src/ptr/mod.rs:2490,2532`, in the `rust-src` tree at `lib/rustlib/src/rust/library/`) and rustc DOES mono-collect the addr shim for comparisons — the real reason comparisons don't surface it is that the bridge's instance walk is shallower than rustc's mono closure (surfaces the shim only for a direct user-body call). The const-provenance rejection is a real but SEPARATE gap (fn pointers in consts/statics). Analysis: `.harness/audits/2026-05-23-c2-fnptraddrshim-retirement.md`. Escalation queue 7 → 6.
- 2026-05-23: C1 (OP-003-I, F68 audit proposal) RETIRED by reclassification — NO code. Plan-first flow (`.harness/audits/2026-05-23-c1-enum-const-eval-plan.md`) + Codex review returned NOT_READY and exposed that the F68 audit mis-scoped C1: it framed an isolated const-eval fix at `const_.rs:42-49`, but the verified blocker is the bridge's by-design rejection of ALL const-generic args (`translate_generic_args` at `bridge/ty.rs:743-748`; reverse at `body/materialise.rs:673-684`, coordinated gap with driver-37) PLUS the deliberate scalar-only type-level admissibility that excludes `Const::EnumValue` (`const_value.rs:54-64`). Codex also empirically dumped MIR for `W<V>` on the pinned nightly to confirm the carrier (`UnevaluatedConst` generic arg, rejected at `ty.rs`) and verified `const_eval_resolve_for_typeck` exists while the plan's `ty_const.eval(...)` candidate does NOT. Architect chose option (1): enum/non-scalar const-generics are out of scope for `#![no_std]` driver specs; OP-003-I reclassified `uncovered` → `blocked-bridge-rejects-by-design` (cross-ref Section 6 RJ-029) in the matrix. Broad const-generic support stays a deferred driver-37 gap. Lesson reinforced (after F72's serde-premise catch): const-related audit proposals' fix-hooks are frequently wrong; verify against HEAD + pinned rustc source at plan stage. Escalation queue 8 → 7.
- 2026-05-24: regseq-driver-74 (R2: tuple `.clone()` `ItemMissingInstance`) drafted via plan-first flow + FILED + pushed at `13443cf`. **Probe-first caught the HANDOFF's R2 mis-scope (4th this session after C1/C2/B1)**: the orphan is NOT the tuple CloneShim (which holds its instance) but the field-type's *library* Clone impl `<u32 as Clone>::clone` — a mono-collected real-body `Item` interned but never instantiated, because its only call edge lives in the CloneShim body (which the bridge leaves absent AND rustc optimizes the `(u32,u32)` shim to copy-only). Root cause: **the bridge interns every collected mono-item but only instantiates the ones a translated body reaches**. Plan Codex-reviewed 2 rounds — **round 1 NOT_READY killed the rev-1 "materialise the CloneShim body" fix** (Codex dumped the optimized shim MIR: copy-only, no call edge; would also need a DFS-RPO block-reorder for the non-RPO shim blocks) → rev-2 recommends **Option B-ii** (APPROVED_WITH_MINOR_FIXES). Spec Codex-reviewed 2 rounds (NOT_READY → APPROVED_WITH_MINOR_FIXES): round 1 caught a false "stash-miss impossible" invariant (eager HIR interning means not every source fn comes through the mono walk; reframed miss = honest hard-error for a separate pre-existing out-of-scope class), mandated calling `lookup_or_create_direct_instance` directly (parity, incl. `ensure_cabi_lowering_slots`), and fixed the coarse sidecar predicate; round 2 caught `callsite_fn_sig` being private (needs `pub(crate)` bump). **Filed fix**: Change A stashes `(def_id, rustc args)` at the `MonoItem::Fn` walk keyed by model `DefPathHash`; Change B = new post-construction pass (in `run_bridge` between `:630` and `:641`) that instantiates source bucket-1 zero-instance Function items via the reactive helper; Change C = `callsite_fn_sig` visibility + integration tests + new `FF-016` coverage row. **~Zero regression by construction** (the pass is a no-op on currently-passing runs). Implementer picks up; assessment must include a focused Codex soundness pass on Change B (instance-table / Phase-5 invariant — [[assessment-soundness-pass]]).
- 2026-05-24: regseq-driver-74 (R2) CLOSED by implementer (`666ca26`) — Type-A assessment **APPROVED**, Codex soundness pass **SOUND**. Implementation faithful to spec: Change A stash at the `MonoItem::Fn` walk (keyed by model `DefPathHash` via `def_path_hash(tcx,def_id)`); Change B `instantiate_orphaned_collected_functions` (run_bridge `:646`, after construct-instances, before F4-2 sort + completeness) — targeting predicate mirrors the validator's bucket-1 gates exactly, instances created via `lookup_or_create_direct_instance` + `callsite_fn_sig` (bumped `pub(crate)`); snapshot-orphans-first (translating a FnDef can intern fresh items); hard-error on stash-miss (separate pre-existing class). Change C: FF-016 matrix+sidecar+snippet + `bridge_orphan_collected_instance.rs` (5 tests, real assertions: B1 one `::clone` Item inst body=None, B2 CloneShim still present, B3 nested multi-field, B4 fnptr no-spurious-Item, B5 clean-crate inert). Verification: build ✅, fmt ✅, regseq-driver **lib** clippy ✅, bridge-coverage-runner clippy ✅, regseq-driver tests ✅ (0 fail), new tests ✅ 5/5, Mode A independently re-run **144/144/0** exit 0 with FF-016 PASS and all 143 prior rows passing. **Codex soundness pass (mandated for the instance-table change): VERDICT SOUND** — DUPLICATE_KEY_SAFE PASS (helper dedups by key + R1 backstop), TARGETING_NO_FALSE_POSITIVE PASS (matches validator buckets), ORDERING_SAFE PASS, and the key concern **LOCAL_ORPHAN_BODY_NONE = NOT_REACHABLE** (Codex built a `(MyCopyStruct,u32).clone()` probe: the local derived clone gets a real body via `run_phase5_body_passes` BEFORE the pass runs — `body_none=false`; only the cross-crate `u32` clone is the body=None orphan the pass handles). No latent hole. **Two notes (not blockers):** (a) no `.harness/expectations/regseq-driver-74.md` shipped (same as driver-69; not a spec AC — convention deviation only); (b) `cargo clippy -p regseq-driver --all-targets` fails with MANY PRE-EXISTING test-target lints (clippy-version drift) across untouched test files — same class as R1's regseq-model 23 errors, now confirmed for regseq-driver too; the implementer's NEW lib code + NEW test file are clippy-clean. Repo-hygiene follow-up: a dedicated clippy-test-target cleanup task ([[clippy-all-targets-test-drift]]). R2 closed; R3 (used-closure clone ICE) next.
- 2026-05-24: regseq-driver-75 (R3: constant args-tuple closure-call ICE) drafted via plan-first flow + FILED + pushed at `80917ab`. **Probe-first caught the B1-audit's R3 mis-scope (5th this session).** B1 framed R3 as a "used cloned closure / CloneShim handling" `fn_sig()`-on-a-closure ICE; the probe disproved both — a plain `let c = move || x; c()` (no clone) ICEs identically, and the path is the closure-**call** rewrite. **Root cause:** a closure `Fn*::call` whose monomorphised args-tuple is an `Operand::Constant` makes `rewrite_closure_call_shim` bail (`terminator.rs:286 _ => Ok(None)`) → falls through to `translate_call_callee` → `callsite_fn_sig` → `tcx.fn_sig(closure)` → ICE (closures need `args.as_closure().sig()`). **Plan Codex-reviewed 2 rounds — round 1 NOT_READY widened the breadth** (not just zero-arg `const ()`; non-empty **all-ZST** args like `c(())` also lower to a constant tuple and hit the same path — proved by a Codex MIR probe) → round 2 APPROVED (materialise-and-project chosen over typed-reject for ZST args; feasibility confirmed via the `cast_receiver_to_captures_view` synthetic-local pattern). **Spec Codex-reviewed 2 rounds** (APPROVED_WITH_MINOR_FIXES → APPROVED): round 1 fixed 6 drifted anchors, mandated `Operand::Move(projected)` + `Rvalue::Use(translate_operand)` for the materialised fields, resolved a B4/AC4 handle-vs-reject contradiction (ZST args MUST bridge Ok), and set the FF-017 predicate to `SyntheticItemKind = ClosureCall`. **Filed fix (Option A)**: handle the constant args-tuple in the rewrite (empty → zero positional args, empirically proven; non-empty all-ZST → materialise into a synthetic local + project fields as `Operand::Move`) and replace the silent bail with a typed `DslRejectionKind::Terminator`. Confined to the rewrite's args-untupling + `FF-017` coverage row + `bridge_zero_arg_closure_call.rs` tests. Quality Gate scopes clippy to lib + the new test (per [[clippy-all-targets-test-drift]]). Implementer picks up; assessment = Type-A + a focused Codex soundness pass on the call-terminator-construction change ([[assessment-soundness-pass]]). Bonus finding: R2's orphan pass already fixed the B1 audit's *unused* cloned-closure case (`let _ = c.clone();`, was `ItemMissingInstance`, now `Ok`).
