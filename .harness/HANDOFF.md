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

- **Last updated**: 2026-05-23
- **Last completed task**: regseq-driver-72 (F71 follow-up trio: SyntheticItemKindSub predicate + ClosureOnceShim Phase-5 binding + RJ-009 bucket-4 carve-out coverage — all 3 sub-tasks PASS, 4/4 rows committed, zero new escalations) at commit `9186248`
- **Last commit on main**: `dc10414` — `docs: retire C1 (OP-003-I) by reclassification after plan-stage misdiagnosis` (**pushed; local in sync with origin**). Recent history: `9186248` (F72 impl) → `dfa5fad` (coverage-drift tool + finding) → `821ec7d` (user: moved Codex runner to a user-level Skill — CLAUDE.md §2 is now "External Review") → `dc10414` (C1 reclassification). Codex is now invoked via the `codex-runner` Skill (verified working 2026-05-23); the local `scripts/python/codex_runner.py` is retired — see §Workflow → Codex invocation.
- **In-flight task**: none — awaiting user direction. Next on the F68 slate is **C2** (FF-013 fn-ptr-provenance); also open: B1, B2, F1, the F70 residuals (FF-005 RustCall ABI, RV-009-G slice), or gap-inventory Category 4 (Section 7 validator audit). **C1 is RETIRED** (reclassified — see below). Suggested next move: a HEAD-grounded feasibility sanity-check on C2 *before* a full plan, given C1/F72 both showed audit proposals can mis-scope the fix-hook.
- **Runner state**: **140 committed sidecar rows**, all PASS via Mode A (Total=140, Pass=140, Fail=0; independently re-verified at assessment). F72 added **2 new InspectionPredicate arms** (`SyntheticItemKindSub { kind, sub }` for the ClosureCall call_kind sub-axis; `SignatureNonRigid` reading `BridgeItemFlags.signature_non_rigid` from the in-process model).
- **KNOWN DRIFT (open; remedy deferred)**: the matrix per-row `status` column understates runner coverage — at HEAD the sidecar covers 140 rows but the matrix marks only 19 of them `covered` (121 still read `uncovered`/`partial`); separately ~94 matrix-`covered` rows are legacy-fixture-only (not in sidecar). Root cause: the bulk F4 corpus committed sidecar rows without backfilling matrix `status`. The matrix `status` is therefore NOT a reliable coverage view. Read-only report at `scripts/python/coverage_drift.py`; full finding at `.harness/audits/2026-05-23-matrix-sidecar-drift.md`. Remedy = derive matrix status from sidecar under Category 5 (do NOT hand-backfill). A per-gap living ledger was sketched and DEFERRED to Category 5 for the same "derive, don't hand-maintain" reason (`.harness/audits/2026-05-23-gap-ledger-proposal.md`).
- **Active escalation queue**: **7 items remaining** (F72 closed FF-007, RJ-009, SK-004, SK-005; **C1/OP-003-I closed-by-reclassification 2026-05-23** — see below). Open: **C2** FF-013 (FnPtrAddrShim provenance), **B1** FF-012 (CloneShim arrays/tuples), **B2** FF-004 + RJ-024 (deferred-shim probe, LOW), **F1** RJ-032-F (watch-list, LOW), plus the F70 residuals **FF-005** (RustCall ABI gap at `bridge/fnsig.rs:107-115`) and **RV-009-G slice** (at `bridge/body/rvalue.rs:197`). Cluster taxonomy below; per-row proposal assignment in `.harness/audits/2026-05-21-post-f4-triage.md` Summary table.
- **C1 (OP-003-I) RETIRED by reclassification, not implemented**: the F68 audit mis-scoped C1 as an isolated const-eval fix. Codex plan review (+ verification) found the real blocker is the bridge's **by-design rejection of all const-generic args** (`translate_generic_args` at `bridge/ty.rs:743-748`; reverse at `body/materialise.rs:673-684`, coordinated gap with driver-37) PLUS the deliberate exclusion of `Const::EnumValue` from type-level admissibility (scalar-only at `const_value.rs:54-64`). Architect decided enum/non-scalar const-generics are out of scope for `#![no_std]` driver specs; OP-003-I reclassified `uncovered` → `blocked-bridge-rejects-by-design` (cross-ref Section 6 RJ-029). Analysis: `.harness/audits/2026-05-23-c1-enum-const-eval-plan.md`. **Broad const-generic support remains a deferred gap (driver-37), to be taken up deliberately only if const-generics become a driver-spec requirement.**
- **Uncommitted state in working tree**: clean except two untracked dirs left alone by standing rule — `tests/fixtures/playground-pl011/` + `crates/regseq-driver/tests/fixtures/` (experimentation/fixtures — never commit, never modify). (The earlier `CLAUDE.md` edit was committed by the user as `821ec7d`.)

### Active escalation cluster taxonomy (driver-68's input)

| Cluster | Affected rows | Root-cause family |
| ------- | ------------- | ------------------ |
| (a) Bridge type-projection normalization | RV-007-C, RV-009-G, FF-005 | `Pointee::Metadata` + `<fn() as FnOnce>::Output` projections don't normalize through concrete types |
| (b) Shim materialization gaps | FF-004 (ReifyShim), FF-012 (CloneShim), FF-013 (FnPtrAddrShim) | Distinct shim families; FF-004 elided by rustc, FF-012 hits tuple/array completeness, FF-013 hits const-eval-over-fn-pointer-provenance |
| (c) Const-eval surface gaps | OP-003-I (Const::EnumValue), FF-013 overlap | `mir::Const::eval` over unevaluated const blocks / fn-pointer provenance |
| (d) Helper / build-contract gaps | TM-006-C (helper overflow_checks=false), RJ-006 (cleanup-pass strips MIR statements), RJ-032-F (target-binding wiring not landed) | Infrastructure / wiring; not bridge bugs |
| (e) Bridge-internal invariant rejections | RJ-024 (UnsupportedEnumLayout) | Rejection paths that rustc's layout solver doesn't produce from source |
| (f) OPEN BUG family pointer | IX-014 | Blocked by IX-004/IX-010 (`Fn::call` resolution in polymorphic MIR) |

Cluster → proposal mapping (per the F68 audit doc):
(a) → A1 HIGH; (b) FF-012 → B1 MEDIUM, (b) FF-004 + (e) RJ-024 → B2 LOW;
(c) OP-003-I → C1 MEDIUM, (c) FF-013 → C2 MEDIUM; (d) TM-006-C + RJ-006 → D1 HIGH,
(d) RJ-032-F → F1 LOW; (f) IX-014 → F1 LOW (bundled with RJ-032-F as "deferred pending external work").

### Next actions (in priority order)

1. **Draft next follow-up spec** from the remaining F68 slate, in audit-doc Closing-recommendations order (C1 now RETIRED by reclassification — see §Current state): **C2** (FF-013 fn-ptr-provenance — next up), **B1** (FF-012 CloneShim — coordinate with C2 const-surface; B1 spec-drafting session MUST empirically pin the fix location per audit-doc Codex warning footnote AND heed the C1 lesson: const-related proposals' fix-hooks are often NOT where the audit says — verify against HEAD + pinned rustc source before drafting), **B2** (FF-004/RJ-024 probe-or-defer LOW), **F1** (RJ-032-F watch-list LOW). Plan → Codex → spec → Codex → file flow per the F72 pattern (F72 plan 3 Codex rounds, spec 3 rounds).
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
- 2026-05-23: C1 (OP-003-I, F68 audit proposal) RETIRED by reclassification — NO code. Plan-first flow (`.harness/audits/2026-05-23-c1-enum-const-eval-plan.md`) + Codex review returned NOT_READY and exposed that the F68 audit mis-scoped C1: it framed an isolated const-eval fix at `const_.rs:42-49`, but the verified blocker is the bridge's by-design rejection of ALL const-generic args (`translate_generic_args` at `bridge/ty.rs:743-748`; reverse at `body/materialise.rs:673-684`, coordinated gap with driver-37) PLUS the deliberate scalar-only type-level admissibility that excludes `Const::EnumValue` (`const_value.rs:54-64`). Codex also empirically dumped MIR for `W<V>` on the pinned nightly to confirm the carrier (`UnevaluatedConst` generic arg, rejected at `ty.rs`) and verified `const_eval_resolve_for_typeck` exists while the plan's `ty_const.eval(...)` candidate does NOT. Architect chose option (1): enum/non-scalar const-generics are out of scope for `#![no_std]` driver specs; OP-003-I reclassified `uncovered` → `blocked-bridge-rejects-by-design` (cross-ref Section 6 RJ-029) in the matrix. Broad const-generic support stays a deferred driver-37 gap. Lesson reinforced (after F72's serde-premise catch): const-related audit proposals' fix-hooks are frequently wrong; verify against HEAD + pinned rustc source at plan stage. Escalation queue 8 → 7.
