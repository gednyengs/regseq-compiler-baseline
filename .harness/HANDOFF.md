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

- **Last updated**: 2026-05-21
- **Last completed task**: regseq-driver-68 (F5 post-F4 batch triage, 7 proposals for human approval) at commit `c3456a5`
- **Last commit on main**: `c3456a5` — `regseq-driver-68: post-F4 batch triage (7 proposals for human approval)` (post-implementer handoff-doc commits `88888f7` and this update follow)
- **In-flight task**: none — awaiting user approval of a subset of F68 triage proposals before drafting regseq-driver-69+ task specs
- **Runner state**: 123 committed sidecar rows, all PASS via Mode A. 22/22 InspectionPredicate dispatch arms. 4/4 sub-axis matcher fields. Direct binary works on macOS via build.rs rpath. **No change from F4 close** (F68 is analytical; no code modifications).
- **Active escalation queue**: 12 items unchanged (11 F4 + 1 F2 IX-014). Now formalized into 7 PROPOSED follow-up tasks in `.harness/audits/2026-05-21-post-f4-triage.md` (A1, B1, B2, C1, C2, D1, F1). Cluster taxonomy below; per-row proposal assignment in the triage doc's Summary table.
- **Uncommitted state in working tree**: `tests/fixtures/playground/src/lib.rs` (user's experimentation surface — never commit, never modify).

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

1. **User reviews triage proposals** at `.harness/audits/2026-05-21-post-f4-triage.md` and approves a subset (recommended order in the doc's Closing recommendations table). Highest-leverage starting set: D1 (HIGH, smallest infra unblock for TM-006-C + RJ-006) and A1 (HIGH, single normalization clears RV-007-C / RV-009-G / FF-005).
2. **Draft follow-up specs** (regseq-driver-69+) per each approved proposal via the spec → Codex → file flow. Each approved proposal is one task spec; F68 deliberately did not pre-author them.
3. **Possible matrix precision patches** as a side track (FF-002 / IX-017 diag-log peel shape mis-classification surfaced in F2/F3 expectations).
4. **Eventual post-discovery strategy items**: CI gating (Mode C), Mode B agent integration, JSON report output. Per strategy §10 these are deferred until corpus stabilises; F4 close + F5 triage close the discovery-engine arc.

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

---

## § Project orientation (STABLE)

- **Regseq compiler**: Rust-to-C device driver compiler. User is the designer/architect.
- **Current workstream**: bridge-coverage-runner — a matrix-coverage discovery engine that turns every uncovered row of `docs/discussions/regseq-model/bridge-coverage-matrix.md` into an executable bridge experiment with deterministic PASS/FAIL.
- **Current phase**: 1a (matches recent commits).
- **Auto-memory**: at `~/.claude/projects/-Users-nyengele-Documents-sekekama-regseq-compiler-asplos/memory/`. Carries role + binding rules: compiler-theory-first, no-defensive-fallbacks, spec-before-implementation, three DSL mechanisms, Codex review loop, playground-is-experimentation-space. Loaded on startup.

---

## § Workflow conventions (STABLE)

### Task spec authoring loop

1. Diagnose → draft spec at `.harness/tasks/regseq-driver/NN-name.md`
2. Send to Codex via `scripts/python/codex_runner.py` with the four-block prompt template
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

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 900
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

Bash `timeout` >= `950000` (Codex `--timeout 900` + margin). Default `--effort high`. Codex empirically probes code (sometimes runs `cargo test`) — trust findings.

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
- `scripts/python/codex_runner.py` — Codex CLI wrapper

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
