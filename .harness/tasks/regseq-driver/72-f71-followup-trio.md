# regseq-driver-72: F71 follow-up trio (SyntheticItemKind sub-variant predicate + ClosureOnceShim Phase-5 binding + RJ-009 source-shape resolution)

## Description

This task bundles the three follow-up gaps surfaced by F71's
escalation records (`.harness/expectations/regseq-driver-71.md:329-487`)
into a single implementer task. The bundle is justified because the
three sub-tasks are independent (no cross-dependency), small enough
that a single Codex review cycle is cheaper than three, and all share
the same triggering context (F71 cohort residuals).

The plan document for this task is at
`.harness/audits/2026-05-23-f71-followup-trio-plan.md` (APPROVED clean
after 3 Codex review rounds: NOT_READY → APPROVED_WITH_MINOR_FIXES →
APPROVED).

**Bundled sub-tasks** (independent; each can PASS or escalate without
blocking the others):

1. **Sub-task A** — runner-precision: add a `SyntheticItemKind`
   sub-variant matching path to the `InspectionPredicate` set so the
   `ClosureCall { call_kind }` axis can be distinguished by predicate.
   Unblocks SK-004 (CallMut) + SK-005 (CallOnce) for sidecar commit.
2. **Sub-task B** — bridge fix: bind the ClosureOnceShim's underlying
   Function Item during synthesis pre-scan, or lazy-allocate it via
   `lookup_or_create_direct_instance`, so the Phase-5 body pass's
   bridge-completeness validation succeeds. Unblocks FF-007 for
   sidecar commit.
3. **Sub-task C** — investigation + decision: resolve RJ-009's
   source-shape problem by either (a) exposing the bridge bucket-4
   carve-out flag (`signature_non_rigid: true`) to the runner's
   outcome model so RJ-009/010/011/012/013 have a surfacing path, OR
   (b) authoring a polymorphic-fn surface that bypasses the carve-out
   and surfaces `NonRigidTy::Param` to the existing rejection oracle.
   Sub-task C has a bounded decision gate (see Change C below) and
   may escalate independently without blocking A/B.

### Why now

F71 (commit `7525ac0`) closed Category 1 (closure-family reattempt
sweep) and recorded four escalations — RJ-009, FF-007, SK-004, SK-005
— spanning three distinct gap classes. None of the four was a F71
defect; each was carved out by F71's explicit non-goals
(`.harness/tasks/regseq-driver/71-closure-family-reattempt-sweep.md:122-159`)
as a future follow-up. F72 picks them up as a coherent bundle because:

- Sub-task A unblocks 2 cohort rows (SK-004, SK-005).
- Sub-task B unblocks 1 cohort row (FF-007) AND closes a NEW bridge
  gap distinct from F70's RustCall ABI.
- Sub-task C resolves the design ambiguity around RJ-009's missing
  surfacing path (5 RJ-* rows potentially unblocked, but only RJ-009
  is in-scope here).

Total potential sidecar growth: 136 → 140 if all sub-tasks PASS.

### Realistic outcome expectation

Honest scoping:

- **Sub-task A** (HIGH confidence): the runner-precision gap is well-
  scoped at `crates/bridge-coverage-runner/src/inspect.rs:50-51`,
  `:168-182`, and `:604-618`. The fix is a small extension to the
  predicate surface. Expected outcome: sidecar +2 (SK-004 + SK-005).
- **Sub-task B** (MEDIUM-HIGH confidence): F71 reproduced the
  Phase-5 `ItemMissingInstance` failure with two concrete source
  shapes (`.harness/expectations/regseq-driver-71.md:383-433`). The
  F62 closure-call shim precedent at
  `crates/regseq-driver/src/bridge/body/terminator.rs:354-369`
  provides a working pattern for path (a) lazy-allocate. The two
  admissible paths (a) and (b) each have a clear template. Expected
  outcome: sidecar +1 (FF-007).
- **Sub-task C** (MEDIUM-LOW confidence): F71's 3-retry sweep
  demonstrated that the bridge's bucket-4 carve-out at
  `crates/regseq-driver/src/bridge/mod.rs:1644-1658` silently absorbs
  every polymorphic source shape attempted so far. The implementer
  may discover that C.2.b is infeasible (no source shape bypasses
  the carve-out) and that C.2.a (runner outcome-model extension) is
  too invasive for a single sub-task; in that case, sub-task C
  escalates independently per the bounded decision gate, and the
  task closes with A + B sidecar growth. Expected outcome:
  sidecar +1 (RJ-009) OR sub-task C escalates.

The Acceptance Criteria below explicitly allow sub-task C
independent escalation without blocking AC-A* or AC-B*.

### Out of scope

- **F68 audit slate items** (B1, B2, C1, C2, F1). Those remain in
  their own future task slots per
  `.harness/audits/2026-05-21-post-f4-triage.md:537-545`.
- **Gap inventory Categories 2-6** (FF-002 audit; F7 discovery
  batch; Section 7 validator audit; F5 automation; matrix-precision
  pass). Those remain in their own future task slots per
  `.harness/audits/2026-05-22-gap-inventory.md`.
- **Refactoring or "improving" any of the touched files** beyond
  the surgical changes each sub-task requires.
- **New matrix rows.** F72 only touches rows already in the matrix
  and the sidecar (SK-004, SK-005, FF-007, RJ-009).
- **RJ-010 / RJ-011 / RJ-012 / RJ-013 row commits.** These are
  explicitly out-of-scope even if sub-task C path (a) makes them
  tractable. They become inputs to a future user-approval gate, not
  in-scope work for this task.
- **Snippet authoring beyond the 4 rows** (SK-004, SK-005, FF-007,
  RJ-009).
- **Any weakening or special-casing of Phase-5 bridge-completeness
  validation.** Bucket 1 reports `ItemMissingInstance` for zero-
  instance Function Items at
  `crates/regseq-model/src/model.rs:3622-3627`; the F72 fix must
  preserve that invariant, not loosen it.
- **Any change to F70's `AuthorizedProjectionFolder`** at
  `crates/regseq-driver/src/bridge/ty.rs:81-143`, the F70
  normalization-prelude call at
  `crates/regseq-driver/src/bridge/ty.rs:55-69`, or the downstream
  `TyKind::Alias` rejection arm at
  `crates/regseq-driver/src/bridge/ty.rs:426`. Sub-task C path (b)
  source-shape probes must not silently depend on or widen F70's
  authorized projection set; if a probe requires broader
  normalization, that belongs in a separate task.
- **NO re-attempt of F70's FF-005 RustCall ABI escalation.** That
  escalation stands; the FnPtrShim sig-translation gap is a separate
  task class distinct from F72's three sub-tasks.
- **NO modifications to `tests/fixtures/playground/` or to any
  `tests/fixtures/playground-*/`** (standing playground rule per
  HANDOFF).
- **NO new task filings via `task.py add-task`** beyond this task's
  self-registration. Rows that escalate become inputs to a future
  user-approval gate, not auto-filed follow-ups.

## Input Context

### Design source

- **F72 plan doc** at
  `.harness/audits/2026-05-23-f71-followup-trio-plan.md` — the
  primary design source. APPROVED clean by Codex (3 review rounds).
- **F71 expectation file** at
  `.harness/expectations/regseq-driver-71.md:329-487` — authoritative
  source for the four escalation records that F72 picks up.

### HEAD bridge state (where the fixes land)

Pinned toolchain: `nightly-2026-02-01-x86_64-unknown-linux-gnu`
(or `-aarch64-apple-darwin`) per `rust-toolchain.toml`.

**Sub-task A surfaces** (read-only references; the fix lands here):

- `crates/bridge-coverage-runner/src/inspect.rs:41-51` —
  `InspectionPredicate` enum declaration with
  `#[serde(tag = "kind", content = "arg", rename_all = "PascalCase")]`.
  Current `SyntheticItemKind(String)` arm at `:50-51`.
- `crates/bridge-coverage-runner/src/inspect.rs:168-182` —
  `synthetic_kind_variant_tag` function that collapses
  `ClosureCall { call_kind: Call / CallMut / CallOnce }` to the
  single tag `"ClosureCall"`.
- `crates/bridge-coverage-runner/src/inspect.rs:604-618` — dispatch
  arm for `InspectionPredicate::SyntheticItemKind` that matches only
  on the variant tag.

**Sub-task B surfaces**:

- `crates/regseq-driver/src/bridge/body/terminator.rs:209-214` —
  `LangItem::Fn` / `FnMut` / `FnOnce` mapping to `ClosureCallKind`
  inside `synthesise_closure_call_shim`'s rewrite.
- `crates/regseq-driver/src/bridge/body/terminator.rs:340-371` —
  F62 closure-call shim `lookup_or_create_direct_instance` precedent
  for path (a) lazy-allocate.
- `crates/regseq-driver/src/bridge/body/terminator.rs:559-566` —
  analogous call-site path's `lookup_or_create_direct_instance` use.
- `crates/regseq-driver/src/bridge/body/materialise.rs:335-389` —
  Phase-5 body materialization surface where the
  `ItemMissingInstance` failure surfaces.
- `crates/regseq-model/src/model.rs:3622-3627` — bucket 1
  bridge-completeness validation that reports `ItemMissingInstance`
  when count is zero.

**Sub-task C surfaces**:

- `crates/regseq-driver/src/bridge/mod.rs:1637-1660` — bucket-4
  carve-out that captures `BridgeError::NonRigidTy { .. }`, sets
  `BridgeItemFlags { signature_non_rigid: true, .. }`, and pushes
  no Instance.
- `crates/regseq-model/src/item.rs:207-224` — `BridgeItemFlags`
  struct declaration. The struct itself does NOT derive
  `Serialize` / `Deserialize` at HEAD; `signature_non_rigid` is a
  plain `bool` field at `:213-223`.
- `crates/regseq-model/src/model.rs:85-94` — the side-table field
  `RegseqModel::item_bridge_flags: HashMap<ItemId, BridgeItemFlags>`
  that holds the bridge-only flags. The field carries
  `#[serde(skip)]` at `:93`, with a doc comment at `:90-92` stating
  "Not serialised: the bridge repopulates it on every run, and the
  model-correctness validate entry point does not consult it." The
  runner's deserialized model therefore sees an empty
  `item_bridge_flags` map.
- `crates/regseq-model/src/model.rs:3612-3620` — bucket-4
  validation that accepts zero-instance Function Items.
- `crates/bridge-coverage-runner/src/main.rs:191-197` — positive-row
  inspection path: only runs against `OracleOutcome::Ok(model)`.
- `crates/bridge-coverage-runner/src/main.rs:207-220` — rejected-row
  expectation matching: only matches against `BridgeError`.
- `crates/bridge-coverage-runner/src/main.rs:223-230` — a rejected
  row that produced `Ok` fails outright.

**Spec authority for sub-task B path choice**:

- `docs/discussions/regseq-model/spec.md:2525-2710` — §9.3-9.4
  instance/flavor identity.
- `docs/discussions/regseq-model/spec.md:4579-4614` — closure-call
  synthesis.

### F71 expectation evidence (the four escalations F72 picks up)

- **RJ-009** at `.harness/expectations/regseq-driver-71.md:331-381`
  — bucket-4 absorption documented; three retry shapes attempted;
  follow-up directions enumerated.
- **FF-007** at `.harness/expectations/regseq-driver-71.md:383-433`
  — Phase-5 `ItemMissingInstance` documented; two failing source
  shapes; root cause + two implementation paths enumerated.
- **SK-004** at `.harness/expectations/regseq-driver-71.md:435-470`
  — runner predicate-precision gap documented; follow-up direction
  enumerated.
- **SK-005** at `.harness/expectations/regseq-driver-71.md:472-487`
  — same root cause as SK-004 for CallOnce sub-variant.

### Existing sidecar baseline + matrix anchors

- **Sidecar baseline at HEAD**: 136 rows committed
  (`.harness/HANDOFF.md` post-F71 close; verified via Mode A green
  at F71 commit `7525ac0`).
- **Matrix doc anchors** at
  `docs/discussions/regseq-model/bridge-coverage-matrix.md`:
  - SK-004 row anchor.
  - SK-005 row anchor.
  - FF-007 row (current status:
    `blocked-bridge-itemmissinginstance`, introduced by F71 per
    `.harness/expectations/regseq-driver-71.md` §B-6 reclassification).
  - RJ-009 row anchor (current status: `partial` per F71's
    "retained at `partial` rather than escalated to `uncovered`"
    discipline at
    `.harness/expectations/regseq-driver-71.md:374-378`).

The implementer should grep the matrix doc for each row id to locate
the exact anchor lines at the time of implementation (citations may
drift after F71's edits).

### F70 precedent

F70 (commit `4c1332e`) extended bridge type-translation with
`AuthorizedProjectionFolder` and surfaced FF-005 RustCall ABI
escalation. F72 sub-task B's path (a) lazy-allocate mirrors F62's
closure-call shim pattern; F72 does NOT extend
`AuthorizedProjectionFolder` (out-of-scope).

### Prior tasks

- `.harness/tasks/regseq-driver/68-post-f4-batch-triage.md` (F68 audit)
- `.harness/tasks/regseq-driver/70-bridge-associated-type-projection-normalization.md` (A1)
- `.harness/tasks/regseq-driver/71-closure-family-reattempt-sweep.md` (Category 1)
- `.harness/audits/2026-05-21-post-f4-triage.md` (F68 audit body)
- `.harness/audits/2026-05-22-gap-inventory.md` (gap inventory)
- `.harness/audits/2026-05-23-f71-followup-trio-plan.md` (this task's plan)

## Implementation Notes

The three sub-tasks have no hard dependencies. Recommended order
inside the single F72 task: **A → B → C**. If any sub-task escalates,
the others proceed unaffected.

### Change A: `SyntheticItemKind` sub-variant predicate

**Problem**: `InspectionPredicate::SyntheticItemKind(String)` at
`crates/bridge-coverage-runner/src/inspect.rs:50-51` matches only
the variant tag returned by `synthetic_kind_variant_tag` at
`:168-182`, which collapses `ClosureCall { call_kind: Call }`,
`{ CallMut }`, and `{ CallOnce }` to the single tag `"ClosureCall"`.
SK-003 (committed by F71 with `arg = "ClosureCall"`) would falsely
"cover" SK-004 and SK-005 if they were committed.

**Path choice** (the implementer chooses):

- **Shape A.2.ii (RECOMMENDED)** — keep the existing
  `SyntheticItemKind(String)` arm untouched and add a sibling arm,
  e.g.,
  ```rust
  /// At least one `SyntheticItemKind` entry matches the named
  /// outer variant AND the named sub-axis (e.g.,
  /// `ClosureCall { call_kind: CallMut }`).
  SyntheticItemKindSub { kind: String, sub: String },
  ```
  No backward-compatibility risk: every existing row continues to
  deserialize unchanged. SK-004 / SK-005 use the new arm; SK-003 and
  any other `ClosureCall` rows keep the old arm.

- **Shape A.2.i (ALLOWED with conditions)** — change the existing
  arm from `SyntheticItemKind(String)` to
  `SyntheticItemKind { kind: String, sub: Option<String> }`. Serde
  constraint: the enum's `#[serde(tag = "kind", content = "arg")]`
  attribute means re-shaping the variant to a struct would normally
  force every existing row's `arg = "ClosureCall"` (a string) to
  become `arg = { kind = "ClosureCall", sub = "..." }` (a struct).
  Existing rows at e.g.
  `crates/bridge-coverage-runner/matrix_metadata/rows.toml:1387` +
  `:1457` would fail to deserialize. To choose A.2.i, the
  implementer MUST:
  - Implement a backward-compat parser shim (e.g., a custom
    deserializer or a `#[serde(untagged)]` wrapper) that accepts
    BOTH the legacy string form AND the new struct form.
  - Document the deserializer design in the implementer's
    expectation file with serde-trait verification.
  - Run the full sidecar-load regression sweep per AC-A3 below.

- **Shape A.2.iii (DISALLOWED absent justification)** — encoding a
  sub-axis into the existing `String` via a delimiter (e.g.,
  `"ClosureCall/CallMut"`) is not admissible because it couples
  encoding rules into matrix-row TOML data, creating a
  maintainability hazard. The implementer may NOT choose A.2.iii
  unless they file written justification with the user before
  implementing.

The implementer's expectation file MUST document which path was
chosen and why.

**Dispatch + tag updates**:

When sub-variant matching is active (i.e., predicate carries a non-
None / non-empty `sub` argument), the dispatch must:

1. Match the parent tag via the existing `synthetic_kind_variant_tag`
   function.
2. Additionally interrogate the matched entry's inner enum for the
   named sub-variant via a new helper. Example shape:
   ```rust
   /// Sub-variant tag for `SyntheticItemKind` values that carry an
   /// inner discriminator. Returns `None` for variants without a
   /// meaningful sub-axis.
   fn synthetic_kind_subvariant_tag(sk: &SyntheticItemKind) -> Option<&'static str> {
       match sk {
           SyntheticItemKind::ClosureCall { call_kind } => Some(match call_kind {
               ClosureCallKind::Call => "Call",
               ClosureCallKind::CallMut => "CallMut",
               ClosureCallKind::CallOnce => "CallOnce",
           }),
           _ => None,
       }
   }
   ```
   The helper lives in `crates/bridge-coverage-runner/src/inspect.rs`
   adjacent to `synthetic_kind_variant_tag`.

3. Dispatch at `crates/bridge-coverage-runner/src/inspect.rs:604-618`
   (or wherever the chosen path locates the new arm) tests:
   `synthetic_kind_variant_tag(sk) == kind_needle &&
    synthetic_kind_subvariant_tag(sk) == Some(sub_needle)`.

The implementer should pattern-match the existing
`InspectionPredicate::FunctionInstanceFlavor` dispatch shape at
`crates/bridge-coverage-runner/src/inspect.rs:585-602` (or
equivalent) for error-formatting conventions.

**Sidecar additions** (after the predicate lands):

- **SK-004 snippet** at
  `crates/bridge-coverage-runner/snippets/SK-004.rs`. Source shape:
  capture-mutating closure passed as a closure-call-shim receiver.
  Reuse the FT-013 source-shape pattern at
  `crates/bridge-coverage-runner/snippets/FT-013.rs` (already
  committed by F71) which routes through `LangItem::FnMut →
  ClosureCallKind::CallMut` at
  `crates/regseq-driver/src/bridge/body/terminator.rs:211`. The
  SK-004 snippet must be a fresh file with its own entry function,
  not a copy of FT-013 — duplicate snippets are caught by the
  drift check.

  `rows.toml` predicate for SK-004 (Shape A.2.ii form):
  ```toml
  inspect = { kind = "SyntheticItemKindSub", arg = { kind = "ClosureCall", sub = "CallMut" } }
  ```
  (Shape A.2.i form would be:
   `inspect = { kind = "SyntheticItemKind", arg = { kind = "ClosureCall", sub = "CallMut" } }`.)

- **SK-005 snippet** at
  `crates/bridge-coverage-runner/snippets/SK-005.rs`. Source shape:
  move-FnOnce capture. Reuse the IX-014 or FT-014 source-shape
  pattern (committed by F71) which routes through `LangItem::FnOnce
  → ClosureCallKind::CallOnce` at
  `crates/regseq-driver/src/bridge/body/terminator.rs:212`.

  `rows.toml` predicate for SK-005 (Shape A.2.ii form):
  ```toml
  inspect = { kind = "SyntheticItemKindSub", arg = { kind = "ClosureCall", sub = "CallOnce" } }
  ```

The new snippets are positive rows; their `expected_outcome` is
`bridge_ok` and the model's `item_synthetic_kinds` must contain a
matching `ClosureCall { call_kind: ... }` entry.

### Change B: ClosureOnceShim Phase-5 binding

**Problem**: Fn-as-FnOnce or FnMut-as-FnOnce coercion shapes trigger
rustc to synthesize an `InstanceKind::ClosureOnceShim` at
monomorphization. The bridge maps this through
`function_instance_flavor_for_kind` to
`FunctionInstanceFlavor::ClosureOnceShim`, but the Phase-5 body
pass's bridge-completeness validation fails because the shim
Instance's body materialization references an Item that the bridge
has not yet bound to the model.

F71's repro at
`.harness/expectations/regseq-driver-71.md:397-405`:

```
ItemMissingInstance { item: ItemId(6), item_kind: Function }
```

This is a NEW bridge gap distinct from F70's RustCall ABI rejection
at `crates/regseq-driver/src/bridge/fnsig.rs:107-115` (which fires
BEFORE ClosureOnceShim materialization). FF-007's failure is on the
materialization side, once the Phase-5 body pass attempts to complete
the shim Instance.

**Path choice** (the implementer chooses):

- **Path B.2.a — lazy-allocate** the Function Item the shim
  references through `lookup_or_create_direct_instance`, mirroring
  the F62 closure-call shim pattern at
  `crates/regseq-driver/src/bridge/body/terminator.rs:354-369` and
  the analogous call-site path at `:559-566`. The implementer must
  verify the cache-key shape matches whatever eager-construction
  the rest of the bridge uses (or else create a duplicate
  Instance — see Risk R-1 in the plan doc).

- **Path B.2.b — pre-scan bind** the ClosureOnceShim's underlying
  Item during the synthesis pre-scan, so by the time Phase-5
  validation runs, the Item is already in the model.

**No third path is admissible**: weakening or special-casing
Phase-5 bridge-completeness validation to tolerate the missing
Item/Instance is NOT allowed. Bucket 1 validation requires exactly
one instance and reports `ItemMissingInstance` when count is zero
at `crates/regseq-model/src/model.rs:3622-3627`; relaxing that
invariant for ClosureOnceShim would break the bridge-completeness
contract that the rest of the materialization surface depends on.
The fix must preserve instance completeness, not loosen it.

The implementer chooses between paths (a) and (b) based on which is
more spec-faithful per:

- `docs/discussions/regseq-model/spec.md:2525-2710` (§9.3-9.4
  instance/flavor identity)
- `docs/discussions/regseq-model/spec.md:4579-4614` (closure-call
  synthesis)
- `crates/regseq-driver/src/bridge/body/materialise.rs:335-389`
  (Phase-5 body materialization surface)
- `crates/regseq-driver/src/bridge/body/terminator.rs:354-369`
  (F62 closure-call shim lazy-allocate precedent)

The decision MUST be documented in the implementer's expectation
file with citations to whichever spec/code authorities the
implementer used to choose.

**Cache-key invariant (path B.2.a)**: if the implementer chooses
path (a), they MUST document in the expectation file:

- Which cache-key shape `(ItemId, ArgsKey, FunctionInstanceFlavor)`
  is used for the lazy allocation.
- Why that key shape matches (or differs from) the eager-construction
  surface elsewhere in the bridge.
- A verification step demonstrating that no duplicate Instance is
  created (e.g., a `cargo test` integration test, or a Mode A
  delta-check comparing pre-fix vs post-fix instance counts on a
  representative input).

**Sidecar addition** (after the fix lands):

- **FF-007 snippet** at
  `crates/bridge-coverage-runner/snippets/FF-007.rs`. Source shape:
  the Fn-as-FnOnce coercion form at
  `.harness/expectations/regseq-driver-71.md:397-401`:

  ```rust
  fn call_once<F: FnOnce(i32) -> i32>(f: F, x: i32) -> i32 { f(x) }
  pub fn entry() -> i32 {
      let c = |n: i32| n + 1;
      call_once(c, 5)
  }
  ```

  `rows.toml` predicate for FF-007 is unchanged from its current
  shape (the F71 row entry, if any, may have used
  `FunctionInstanceFlavor=ClosureOnceShim`). The implementer
  verifies the exact predicate against the existing row entry.

### Change C: RJ-009 source-shape resolution (bounded gate)

**Problem**: RJ-009's matrix row predicate requires a
`NonRigidTy::Param` rejection to be observable from a Rust source
shape. F71's 3-retry sweep
(`.harness/expectations/regseq-driver-71.md:333-348`) demonstrated
that the bridge's bucket-4 carve-out at
`crates/regseq-driver/src/bridge/mod.rs:1637-1660` silently absorbs
the polymorphic `Function` item's `translate_fn_sig` error, setting
`BridgeItemFlags { signature_non_rigid: true, .. }` and pushing no
Instance. The bridge therefore does NOT surface a `NonRigidTy::Param`
rejection from any F71-cohort source shape.

**Path choice** (the implementer chooses one):

- **Path C.2.a — runner-side outcome-model extension**. The bridge
  records `BridgeItemFlags.signature_non_rigid` but the runner does
  not consume the flag because:
  - The flag is currently `#[serde(skip)]` at
    `crates/regseq-model/src/model.rs:85-94` (bridge-only,
    not exposed to deserialized model).
  - Bucket-4 validation accepts zero-instance Function Items at
    `crates/regseq-model/src/model.rs:3612-3620`, so the row
    produces `OracleOutcome::Ok(model)` at the runner.
  - Positive-row inspection only runs against `Ok(model)` at
    `crates/bridge-coverage-runner/src/main.rs:191-197`; rejected
    rows only match `BridgeError` at `:207-220`; a rejected row
    producing `Ok` fails outright at `:223-230`.

  Therefore C.2.a is NOT just a one-line predicate addition. The
  implementer must explicitly choose ONE of:

  - **C.2.a.i — keep RJ-009 as a positive row.** Expose the
    bucket-4 flag via serde. The correct serde surface is:
    - `crates/regseq-model/src/item.rs:207-224` —
      `BridgeItemFlags` struct does NOT derive
      `Serialize`/`Deserialize` at HEAD.
    - `crates/regseq-model/src/model.rs:90-94` — the side-table
      field `RegseqModel::item_bridge_flags: HashMap<ItemId,
      BridgeItemFlags>` carries `#[serde(skip)]` at `:93`.

    The implementer must EITHER:
    - **(α) Expose the existing side-table directly**: add
      `Serialize` / `Deserialize` derives to `BridgeItemFlags` at
      `item.rs:207`, AND remove the `#[serde(skip)]` annotation at
      `model.rs:93` so `item_bridge_flags` round-trips through
      JSON / TOML. (Note: the comment at `model.rs:90-92` says the
      bridge repopulates it on every run and the model-correctness
      `validate` entry point does not consult it — removing the
      skip changes the persistence contract; the implementer must
      verify the comment is updated and no downstream consumer
      regresses.) OR
    - **(β) Add a serializable mirror**: introduce a parallel
      `Serialize`-able side-table on `RegseqModel` (e.g.,
      `item_signature_non_rigid: HashMap<ItemId, bool>`) that the
      bridge populates alongside `item_bridge_flags`, leaving the
      original `#[serde(skip)]` in place.

    Then add a new `InspectionPredicate` arm (e.g.,
    `SignatureNonRigid` or `BridgeItemFlag { name, expect_true }`)
    that interrogates the exposed flag on the deserialized model.
    The matrix-row TOML's existing semantic for RJ-009 (currently
    `partial`) must be reframed as "positive-row coverage of the
    bucket-4 carve-out". The implementer's expectation file MUST
    document the chosen serde sub-path (α or β), the semantic
    shift, and the matrix-doc per-row update.

  - **C.2.a.ii — introduce a third outcome class** beyond
    `Ok`/`BridgeError` (e.g., `OracleOutcome::CarveOut(model,
    flags)`). Adds an enum variant to the runner's outcome model
    and a new expectation matcher for the rejection-like signal.
    Larger surface than C.2.a.i; only justifiable if the carve-out
    signal is expected to be re-used across many future rows.

- **Path C.2.b — source-shape engineering**. Find or construct a
  polymorphic-function source shape that bypasses the bucket-4
  carve-out (e.g., a polymorphic trait method whose receiver type
  is not fully bound at the monomorphization site, or a polymorphic
  generic that survives `Instance::try_resolve` without being
  absorbed by bucket-4). Commit RJ-009 with this shape against the
  existing rejection oracle.

**Bounded decision gate** (REQUIRED):

The implementer attempts up to **four** candidate source shapes for
path C.2.b (extending F71's three-retry sweep by one shape). If none
of the four shapes surface a `NonRigidTy::Param` rejection through
the current oracle:

- The implementer either proceeds to path C.2.a (committing to
  C.2.a.i or C.2.a.ii in their expectation file with rationale), OR
- The implementer **escalates sub-task C independently** with
  rationale in the expectation file. Sub-tasks A and B are NOT held
  by C's escalation — their sidecar additions land regardless.

The implementer MUST document in the expectation file:

- Which four candidate source shapes were tried (or fewer if a
  PASS was achieved earlier).
- The bridge error / `Ok` outcome for each.
- The chosen disposition: C.2.a.i, C.2.a.ii, C.2.b PASS, or
  sub-task-C-escalation.

**Sidecar addition** (after the chosen path lands, if not
escalated):

- If **C.2.a**: a new `InspectionPredicate` arm (e.g.,
  `SignatureNonRigid` or `BridgeItemFlag`) plus a snippet at
  `crates/bridge-coverage-runner/snippets/RJ-009.rs` carrying a
  generic-fn surface that triggers the bucket-4 carve-out. RJ-009
  predicate uses the new arm.
- If **C.2.b**: snippet at
  `crates/bridge-coverage-runner/snippets/RJ-009.rs` carrying the
  engineered source shape. The existing rejection-oracle predicate
  continues to work; RJ-009's `expected_outcome` remains
  `bridge_rejection` with a `NonRigidTy::Param` matcher.

### Change D: Matrix-doc per-row status refresh

For each of the four cohort rows that COMMIT to the sidecar, the
matrix doc at
`docs/discussions/regseq-model/bridge-coverage-matrix.md` must be
updated with a refreshed `status` field + notes column entry. The
implementer greps the matrix doc for each row id (SK-004, SK-005,
FF-007, RJ-009) to locate the anchor at implementation time
(citations may drift after F71's edits).

**Reclassification rules** (matching F71's Change C pattern):

- **PASSed rows (committed)**: status becomes `covered`. Notes
  column updated to point at the snippet path. Applies to SK-004,
  SK-005, FF-007 (if Change B succeeds), RJ-009 (if Change C
  succeeds).
- **FF-007 specifically**: if Change B succeeds, the status
  transitions FROM `blocked-bridge-itemmissinginstance` (the new
  class F71 introduced) TO `covered`. The
  `blocked-bridge-itemmissinginstance` status class is NOT
  removed from the matrix doc's status enumeration even if FF-007
  vacates it; that belongs in a separate matrix-precision pass
  (gap inventory Category 6).
- **RJ-009 specifically**: if Change C succeeds via path (a), the
  status transitions FROM `partial` TO `covered`, with notes
  citing the new carve-out predicate; if via path (b), same
  transition with notes citing the engineered source shape; if
  sub-task C escalates per the bounded gate, the status remains
  `partial` with notes column updated to point at the escalation
  rationale in the expectation file. **Plan-refinement note**: the
  plan doc at
  `.harness/audits/2026-05-23-f71-followup-trio-plan.md:404-406`
  used the placeholder status name `escalated-c-bounded-gate` for
  the bounded-gate escalation case. The spec refines this to
  `partial + notes` because introducing a one-row status class for
  the escalation case is overkill and the bounded-gate disposition
  is fully captured in the expectation file. The matrix-doc status
  enumeration is not extended by F72.

No row gets its status silently widened beyond what the empirical
evidence supports.

The matrix-doc edit scope is restricted to:

- Per-row status + notes updates for the (up to) four cohort rows.
- NO collateral matrix edits.
- NO prose refresh outside the per-row updates (the F71-landed
  Part-1 OPEN BUG section refresh is final; F72 does NOT re-edit
  that section).

### Change E: NO unrelated touch

This task does NOT modify:

- F70's `AuthorizedProjectionFolder` or surrounding F70 surface
  (`bridge/ty.rs:55-69`, `:81-143`, `:426`).
- F69's `BuildOverrides` / `RowBuildOverrides` schema.
- F2 / F3 / F4 existing predicate arms (only ADDS one new arm for
  sub-task A; only ADDS one new arm for sub-task C if path (a)
  chosen).
- F71's matrix-doc Part-1 OPEN BUG refresh prose at the existing
  section (the refresh is final).
- `tests/fixtures/playground/` or `tests/fixtures/playground-*/`
  (standing playground rule).

## Non-Goals

- **NO new task filings** via `task.py add-task`. Sub-task C's
  bounded-gate escalation, if it fires, becomes an input to a
  future user-approval gate, NOT an auto-filed follow-up. RJ-010
  / RJ-011 / RJ-012 / RJ-013 row commits (potentially unblocked
  by C.2.a) are explicitly out-of-scope.
- **NO bridge changes beyond sub-task B's ClosureOnceShim binding.**
  Sub-task A is runner-side only. Sub-task C path (a) may require
  exposing a `BridgeItemFlags` field through serde, but does NOT
  touch the bridge's translation surface itself.
- **NO Phase-5 validation weakening**. The `ItemMissingInstance`
  invariant at `crates/regseq-model/src/model.rs:3622-3627` must
  be preserved.
- **NO AuthorizedProjectionFolder changes**. C.2.b probes must not
  silently depend on or widen F70's authorized projection set.
- **NO new `OracleOutcome` variants** unless C.2.a.ii is the
  explicitly-chosen path AND the implementer documents the
  rationale in the expectation file.
- **NO matrix-doc edits beyond the four cohort row per-row
  updates** (Change D). F71's OPEN BUG refresh is final.
- **NO snippet authoring beyond the 4 rows** (SK-004, SK-005,
  FF-007, RJ-009).
- **NO sidecar entries for RJ-010/011/012/013** even if Change C
  path (a) makes them tractable.
- **NO modifications to `tests/fixtures/playground/` or to any
  `tests/fixtures/playground-*/`** (standing playground rule).
- **NO commit / push without user authorization** (standing rule).

## Test Plan

### B1: workspace stays green

`RUST_TEST_THREADS=1 cargo test --workspace -- --test-threads=1`
green at every sub-task boundary (i.e., after Change A lands, after
Change B lands, after Change C lands). Sub-task A modifies
`bridge-coverage-runner`; sub-task B modifies `regseq-driver`'s
bridge; sub-task C may modify either or both depending on path. The
workspace test outcome must remain green throughout.

### B2: runner end-to-end Mode A reports

Manual Mode A invocation after each sub-task lands:

```
target/debug/bridge-coverage-runner \
  --metadata crates/bridge-coverage-runner/matrix_metadata/rows.toml \
  --snippets crates/bridge-coverage-runner/snippets/ \
  --matrix-md docs/discussions/regseq-model/bridge-coverage-matrix.md \
  --out /tmp/brcr-72/
```

Expected: exit 0; `Total = 136 + N` where N is the count of
F72-committed cohort rows; `Fail = 0`. The implementer records the
exact N in the expectations file and the matrix doc's per-row
updates.

Sub-row arithmetic:

- After Change A (sub-task A): N grows by 0 or 2 (SK-004 + SK-005).
- After Change B (sub-task B): N grows by additional 0 or 1
  (FF-007).
- After Change C (sub-task C): N grows by additional 0 or 1
  (RJ-009).

Maximum: N = 4 (sidecar 140). Minimum: N = 0 (all three sub-tasks
escalate; sidecar stays at 136 and the task closes documenting
each sub-task's escalation rationale).

### B3: per-row PASS verification

For each F72-committed cohort row, the runner's per-row report MUST
show PASS against the row's `inspect` predicate (positive rows:
SK-004, SK-005, FF-007, RJ-009 under path C.2.a) or against the
`expected_rejection` shape (RJ-009 under path C.2.b).

### B4: sub-task A regression check

After Change A lands, the implementer runs the full sidecar-load
sweep:

- If **A.2.ii** chosen: only the FT-013 row's
  `SyntheticItemKind=ClosureCall` predicate needs to remain green.
  No other sidecar rows are affected by the new sibling arm.
- If **A.2.i** chosen: EVERY existing `SyntheticItemKind` sidecar
  row (including but not limited to the rows at
  `crates/bridge-coverage-runner/matrix_metadata/rows.toml:1387` +
  `:1457`, plus the F71-committed FT-013 closure-call row) must
  deserialize AND inspect green under the backward-compat parser
  shim. A full Mode A sweep is the verification step.

### B5: sub-task B regression check

After Change B lands, the existing closure-call shim rows committed
by F71 (BD-007, FT-011-014, IX-004, IX-010, IX-014, SK-003) MUST
remain green. The closure-call shim rewrite at
`crates/regseq-driver/src/bridge/body/terminator.rs:150-165` +
`:178-184` is a separate code path from the ClosureOnceShim fix;
F72 must not regress it.

The implementer compares a fresh pre-Change-B Mode A run against
post-Change-B Mode A run on the same input. Per-row PASS / FAIL for
the 136 pre-F72 rows must be identical.

### B6: sub-task C path-disposition documentation

The implementer's expectation file MUST contain:

- The chosen path: C.2.a.i, C.2.a.ii, C.2.b PASS, or
  sub-task-C-escalation.
- Per-candidate-shape outcomes for C.2.b probes (up to 4 shapes).
- For C.2.a.i / C.2.a.ii: the runner-outcome-model decision
  rationale.
- For C.2.b: the source shape, bridge `Ok` outcome, and predicate
  verification.
- For sub-task-C-escalation: the rationale and the proposed shape
  of a future follow-up task (without filing via `task.py
  add-task`).

### B7: matrix-doc per-row status refresh

For each cohort row that commits to sidecar (up to 4 rows), the
matrix doc entry's status field is updated per Change D's
reclassification rules. The matrix doc edit MUST be confined to
those per-row updates; no collateral edits.

If sub-task C escalates per the bounded gate, RJ-009's matrix row
status remains `partial` with notes column updated to point at the
escalation rationale.

### B8: drift check still passes

The matrix sidecar drift check (subset-OK) reports clean: all N
committed cohort rows are documented in the matrix doc; the matrix
doc's per-row updates are consistent with the sidecar.

### B9: no regression on the existing 136 rows

Mode A output's per-row PASS / FAIL classification for the 136
pre-F72 rows is identical pre-Change vs post-Change for each
sub-task. The implementer captures a baseline at the start of the
task and compares against each sub-task close.

### B10: cache-key verification for sub-task B path (a)

If the implementer chose Path B.2.a (lazy-allocate via
`lookup_or_create_direct_instance`):

- The implementer verifies that no duplicate Instance is created
  for the ClosureOnceShim. Verification mechanism: a Mode A delta-
  check OR a `cargo test` integration test that asserts the
  Instance count for the FF-007 input is exactly 1 for the
  ClosureOnceShim flavor.
- The cache-key shape used MUST be documented in the expectation
  file per the Cache-key invariant note in Change B.

### B11: no Phase-5 validation weakening (sub-task B)

The implementer verifies that `crates/regseq-model/src/model.rs`'s
bucket 1 validation at `:3622-3627` is NOT modified. The fix is on
the bridge side (binding the underlying Item), not on the
validator side.

A `git diff --stat regseq-model/` post-Change-B should show NO
modifications to `model.rs` (unless sub-task C path (a) requires
exposing a `BridgeItemFlags` field — see Change C, in which case
the model edit is confined to the `#[serde(skip)]` removal + any
parallel serializable field).

### B12: no AuthorizedProjectionFolder regression

Per the Risk R-5 in the plan doc: sub-task C path (b) source-shape
probes must not silently widen F70's authorized projection set. The
implementer verifies that `crates/regseq-driver/src/bridge/ty.rs`'s
`AuthorizedProjectionFolder` at `:81-143` and the rejection arm at
`:426` are NOT modified.

## Review

### Files to send to Codex (`scripts/python/codex_runner.py`)

- This file (`.harness/tasks/regseq-driver/72-f71-followup-trio.md`).
- The F72 plan doc (`.harness/audits/2026-05-23-f71-followup-trio-plan.md`).
- The F71 expectation file
  (`.harness/expectations/regseq-driver-71.md`) — escalation source.
- The F71 task spec
  (`.harness/tasks/regseq-driver/71-closure-family-reattempt-sweep.md`)
  — non-goals + escalation discipline precedent.
- The F70 task spec
  (`.harness/tasks/regseq-driver/70-bridge-associated-type-projection-normalization.md`)
  — bridge-change precedent (sub-task B's path-a uses F70's
  lazy-allocate pattern indirectly via F62). F70's expectation
  file is NOT in `.harness/expectations/`; F70's escalation
  evidence (FF-005 RustCall ABI) is captured in F71's expectation
  file at `.harness/expectations/regseq-driver-71.md:415-430`.
- The F68 audit (`.harness/audits/2026-05-21-post-f4-triage.md`) —
  remaining slate context.
- The gap inventory (`.harness/audits/2026-05-22-gap-inventory.md`)
  — strategic context.
- The matrix doc (`docs/discussions/regseq-model/bridge-coverage-matrix.md`)
  — refresh target (Change D).
- The runner inspect surface
  (`crates/bridge-coverage-runner/src/inspect.rs`) — sub-task A
  + sub-task C path (a) edit target.
- The closure-call shim surface
  (`crates/regseq-driver/src/bridge/body/terminator.rs`) — sub-task B
  precedent (F62 lazy-allocate pattern at `:354-369`).
- The Phase-5 materialization surface
  (`crates/regseq-driver/src/bridge/body/materialise.rs`) — sub-task B
  failure surface at `:335-389`.
- The bucket-4 carve-out
  (`crates/regseq-driver/src/bridge/mod.rs`) — sub-task C path (a)
  source surface at `:1637-1660`.
- The model bridge flags
  (`crates/regseq-model/src/model.rs`) — sub-task C path (a) serde
  surface at `:85-94`.
- The runner main entry
  (`crates/bridge-coverage-runner/src/main.rs`) — sub-task C
  outcome-model surface at `:191-230`.

### Reviewer focus

- **Bundle granularity**: is the three-sub-task bundle correct? Or
  should sub-task C be split out into a separate task given its
  uncertain outcome?
- **Sub-task A predicate-shape recommendation**: is the A.2.ii
  recommendation correct given the serde adjacently-tagged
  constraint? Does the A.2.i conditional admission protect
  backward compatibility?
- **Sub-task B implementation paths**: are (a) and (b) the only
  admissible paths? Is the Phase-5 weakening exclusion correctly
  framed? Does the cache-key invariant note correctly anticipate
  the F62 precedent's pitfall?
- **Sub-task C bounded gate**: is the 4-shape probe limit
  reasonable? Does the bounded-gate escalation correctly preserve
  A/B sidecar growth?
- **C.2.a.i vs C.2.a.ii decision framework**: does the spec give
  the implementer enough guidance to choose between exposing
  `signature_non_rigid` via serde vs introducing a new
  `OracleOutcome` variant?
- **Out-of-scope discipline**: are the F68 / Categories 2-6 /
  Phase-5 weakening / AuthorizedProjectionFolder exclusions
  correctly framed? Does the spec foreclose unilateral expansion
  of the touched surfaces?
- **Matrix-doc edit scope**: is the per-row update scope
  correctly bounded to the 4 cohort rows? Does it correctly
  exclude collateral edits?
- **Citation drift at HEAD**: HEAD is `c113f86` (post-F71 + HANDOFF
  refresh) at time of spec authoring.
- **Sidecar arithmetic**: 136 + N where N ∈ {0, 1, 2, 3, 4}. Is
  the math correct? Does the per-sub-task increment table
  correctly enumerate the path outcomes?
- **No new task filings**: every escalation routes back to user-
  approval, never to `task.py add-task`.
- **Standing playground rule**: spec correctly excludes
  `tests/fixtures/playground/` and `tests/fixtures/playground-*/`.

### Adversarial prompt template

```
<task>
Review .harness/tasks/regseq-driver/72-f71-followup-trio.md.

This is the proposed task spec for F71's three follow-up gaps:
SyntheticItemKind sub-variant predicate (sub-task A), ClosureOnceShim
Phase-5 binding (sub-task B), RJ-009 source-shape resolution (sub-task
C). The plan document at
.harness/audits/2026-05-23-f71-followup-trio-plan.md was APPROVED
clean by Codex (3 rounds).

Identify defects under the following adversarial checks:

1. BUNDLE GRANULARITY: is the three-sub-task bundle defensible? Should sub-task C be split out into its own task given its uncertain outcome?

2. SUB-TASK A PREDICATE-SHAPE: is the A.2.ii recommendation correct given the adjacently-tagged serde shape at inspect.rs:41-51? Does A.2.i's backward-compat-shim requirement protect existing rows.toml entries at lines 1387 + 1457? Are there other existing SyntheticItemKind rows the spec missed?

3. SUB-TASK B PATH ADMISSIBILITY: are paths (a) lazy-allocate and (b) pre-scan bind the only admissible options? Is the "no Phase-5 weakening" exclusion correctly grounded at model.rs:3622-3627? Does the cache-key invariant note correctly anticipate the F62 closure-call shim precedent's pitfall at terminator.rs:354-369?

4. SUB-TASK C BOUNDED GATE: is the 4-shape probe limit reasonable given F71 already tried 3 shapes? Does the bounded-gate escalation correctly preserve A/B sidecar growth (i.e., A and B land even if C escalates)?

5. C.2.a.i vs C.2.a.ii FRAMING: does the spec give the implementer enough guidance to choose between exposing signature_non_rigid via serde (model.rs:85-94) vs introducing a new OracleOutcome variant (main.rs:191-230)? Are both paths' implications correctly described?

6. OUT-OF-SCOPE DISCIPLINE: are F68 slate / Categories 2-6 / Phase-5 weakening / AuthorizedProjectionFolder (bridge/ty.rs:55-69, 81-143, 426) exclusions correctly framed?

7. MATRIX-DOC EDIT SCOPE: does the spec correctly restrict edits to per-row status updates for the 4 cohort rows? Does it correctly foreclose collateral edits and re-editing F71's OPEN BUG refresh?

8. CITATION DRIFT AT HEAD (c113f86). Verify every file:line cited in the spec.

9. SIDECAR ARITHMETIC: 136 + N where N ∈ {0, 1, 2, 3, 4}. Per-sub-task increment table at B2: Change A → +0 or +2, Change B → +0 or +1, Change C → +0 or +1. Is the math correct?

10. NEW TASK FILINGS: does the spec correctly forbid task.py add-task invocations for escalation follow-ups (sub-task C's bounded-gate escalation, RJ-010-013)?

11. NO RUSTCALL ABI RE-ATTEMPT: does the spec correctly exclude re-attempting F70's FF-005 RustCall ABI escalation as part of any sub-task?

12. CONTRADICTIONS: does the spec internally contradict itself anywhere (e.g., Test Plan vs Implementation Notes vs Acceptance Criteria; out-of-scope vs change description; bounded gate vs ACs)?
</task>

<grounding_rules>
- Cite file:line for every claim against HEAD (`c113f86`).
- For each cited file:line in the task spec, open the file and verify the citation. Quote the source, do not paraphrase the spec's claim.
- Mark hypotheses as hypotheses.
- Pinned toolchain is `nightly-2026-02-01`; HEAD is `c113f86`.
- For SyntheticItemKind precision: read inspect.rs:41-51 (enum), :168-182 (tag function), :604-618 (dispatch).
- For ClosureOnceShim Phase-5: read terminator.rs:209-214 (LangItem mapping), :340-371 (F62 lazy-allocate), materialise.rs:335-389 (Phase-5 surface), model.rs:3622-3627 (bucket 1 validation).
- For bucket-4 carve-out + outcome model: read bridge/mod.rs:1637-1660, model.rs:85-94 (BridgeItemFlags), model.rs:3612-3620 (bucket 4 validation), main.rs:191-230 (outcome dispatch).
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
BUNDLE GRANULARITY: PASS | FAIL with cite
SUB-TASK A PREDICATE-SHAPE: PASS | FAIL with cite
SUB-TASK B PATH ADMISSIBILITY: PASS | FAIL with cite
SUB-TASK C BOUNDED GATE: PASS | FAIL with cite
C.2.a.i vs C.2.a.ii FRAMING: PASS | FAIL with cite
OUT-OF-SCOPE DISCIPLINE: PASS | FAIL with cite
MATRIX-DOC EDIT SCOPE: PASS | FAIL with cite
CITATION DRIFT AT HEAD: list each drifted anchor or "none"
SIDECAR ARITHMETIC: PASS | FAIL with cite
NEW TASK FILINGS: PASS | FAIL with cite
NO RUSTCALL ABI RE-ATTEMPT: PASS | FAIL with cite
CONTRADICTIONS: none | numbered list with cite
NEW ISSUES: numbered list or "none"
REQUIRED FIXES: numbered list (only if VERDICT != APPROVED)
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## Quality Gate

- `cargo fmt --all -- --check` clean.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `RUST_TEST_THREADS=1 cargo test --workspace -- --test-threads=1`
  green.
- Manual Mode A invocation of the runner produces exit 0,
  `Total = 136 + N`, `Pass = 136 + N`, `Fail = 0` (where N ∈ {0, 1,
  2, 3, 4} is the count of F72-committed cohort rows).
- `git diff --stat main..` shows changes confined to:
  - `crates/bridge-coverage-runner/src/inspect.rs` (sub-task A).
  - `crates/bridge-coverage-runner/snippets/SK-004.rs`,
    `SK-005.rs`, `FF-007.rs`, `RJ-009.rs` (sub-task additions; the
    specific files added depend on which sub-tasks land).
  - `crates/bridge-coverage-runner/matrix_metadata/rows.toml`
    (per-row predicate / status updates for the F72-committed
    rows).
  - `crates/regseq-driver/src/bridge/body/terminator.rs` and/or
    `crates/regseq-driver/src/bridge/body/materialise.rs` (sub-task
    B Phase-5 binding fix).
  - `crates/regseq-model/src/item.rs` AND
    `crates/regseq-model/src/model.rs` (sub-task C path (a), if
    chosen — limited to exposing the bucket-4 flag via serde per
    Change C C.2.a.i sub-paths (α) or (β)).
  - `crates/regseq-driver/src/bridge/mod.rs` is editable ONLY if
    sub-path β is chosen, AND only for the tightly-scoped change
    of writing the new serializable mirror alongside the existing
    `item_bridge_flags.insert(...)` at `:1650-1656` inside the
    bucket-4 carve-out. The carve-out's control flow (the
    `BridgeError::NonRigidTy` match arm, the `signature_non_rigid:
    true` assignment, the `continue`) MUST remain unchanged. If
    sub-path α is chosen, NO `bridge/mod.rs` edits are required
    or permitted because removing the `#[serde(skip)]` annotation
    on `item_bridge_flags` exposes the existing recording without
    any bridge-side change.
  - `docs/discussions/regseq-model/bridge-coverage-matrix.md`
    (Change D: per-row status updates for the F72-committed cohort
    rows ONLY).
  - `.harness/expectations/regseq-driver-72.md` (per-sub-task
    outcome record + path-choice rationale).
  - `.harness/HANDOFF.md` (post-task refresh; implementer should
    not edit, the assessor will).
- NO changes to `tests/fixtures/playground/` or
  `tests/fixtures/playground-*/` (standing playground rule).
- NO changes to F70's `AuthorizedProjectionFolder` surface
  (`bridge/ty.rs:55-69`, `:81-143`, `:426`).
- NO changes to `crates/regseq-model/src/model.rs` bucket 1
  validation at `:3622-3627` (sub-task B Phase-5 invariant).
- NO new task filings via `task.py add-task`.
- NO commits or pushes without user authorization (standing rule).

## Acceptance Criteria

The bundle has THREE independent acceptance tiers. Each sub-task's
ACs stand independently; if a sub-task escalates per its bounded-
gate or escalation framework, the OTHER sub-tasks' ACs are not
blocked.

### Sub-task A (SyntheticItemKind sub-variant predicate)

- **AC-A1**: A new sub-variant axis is reachable from the runner's
  `InspectionPredicate` set. The predicate-shape decision (A.2.ii
  recommended, A.2.i admitted with conditions, A.2.iii disallowed
  absent justification) is documented in the implementer's
  expectation file with serde-trait verification.
- **AC-A2**: SK-004 and SK-005 rows are committed to sidecar with
  the new predicate; both PASS under Mode A green.
- **AC-A3**: Regression check. Sufficiency depends on the
  predicate-shape choice:
  - If **A.2.ii** (recommended): the FT-013 snippet's
    `SyntheticItemKind=ClosureCall` row remains green. No further
    sidecar-load coverage required.
  - If **A.2.i**: ALL existing `SyntheticItemKind` sidecar rows
    (including rows at
    `crates/bridge-coverage-runner/matrix_metadata/rows.toml:1387` +
    `:1457`, plus the FT-013 closure-call row) must deserialize and
    inspect green under the backward-compat parser shim. A full
    sidecar-load regression sweep is required.
  - If **A.2.iii**: not applicable (disallowed absent explicit
    justification).
- **AC-A4**: SK-004's matrix-doc row status transitions from its
  current value to `covered` per Change D.
- **AC-A5**: SK-005's matrix-doc row status transitions from its
  current value to `covered` per Change D.

### Sub-task B (ClosureOnceShim Phase-5 binding)

- **AC-B1**: The Phase-5 `ItemMissingInstance` failure no longer
  fires for the FF-007 Fn-as-FnOnce coercion source shape.
- **AC-B2**: FF-007 row is committed to sidecar; PASS under Mode A
  green.
- **AC-B3**: FF-007's matrix-doc row status transitions from
  `blocked-bridge-itemmissinginstance` to `covered` per Change D.
- **AC-B4**: Closure-call shim rows (BD-007, FT-011-014, IX-004,
  IX-010, IX-014, SK-003) remain green (regression check per B5).
- **AC-B5**: If path B.2.a is taken, the implementer's expectation
  file documents:
  - The cache-key shape used for the lazy allocation.
  - The verification step demonstrating no duplicate Instance is
    created (Mode A delta-check OR integration test).
  - Why the chosen cache-key shape matches the eager-construction
    surface elsewhere in the bridge.
- **AC-B6**: NO modifications to `crates/regseq-model/src/model.rs`
  bucket 1 validation at `:3622-3627` (Phase-5 invariant
  preserved).

### Sub-task C (RJ-009 source-shape resolution)

- **AC-C1**: ONE of the following holds:
  - RJ-009 row is committed to sidecar; PASS under Mode A green
    (path C.2.a.i, C.2.a.ii, or C.2.b succeeded), OR
  - Sub-task C escalates per the bounded decision gate, with
    rationale documented in the expectation file. **Sub-task C
    escalation does NOT block AC-A* or AC-B*.**
- **AC-C2**: The implementer's expectation file documents:
  - Which path was taken (C.2.a.i, C.2.a.ii, C.2.b, or escalation).
  - The rationale.
  - The bounded-gate disposition: the up-to-four candidate shapes
    tried for C.2.b (if any), with bridge `Ok` / `BridgeError`
    outcomes for each.
- **AC-C3**: If path C.2.a is taken, RJ-010 / RJ-011 / RJ-012 /
  RJ-013 are NOT automatically committed — those are independent
  rows requiring their own snippets even though the predicate now
  exists.
- **AC-C4**: Matrix doc row for RJ-009 is refreshed per Change D:
  - If path succeeded: status transitions to `covered`.
  - If sub-task C escalated: status remains `partial` with notes
    column updated to point at the escalation rationale.
- **AC-C5**: If path C.2.a is taken, NO modifications to F70's
  `AuthorizedProjectionFolder` surface (`bridge/ty.rs:55-69`,
  `:81-143`, `:426`). If path C.2.b is taken: same constraint —
  C.2.b source-shape probes must not silently depend on or widen
  F70's authorized projection set.

### Cross-cutting

- **AC-X1**: NO new task filings via `task.py add-task`. Sub-task
  C escalation (if it fires) becomes an input to a future
  user-approval gate. RJ-010-013 row commits remain out-of-scope.
- **AC-X2**: `git diff --stat main..` matches the Quality Gate's
  file enumeration. No collateral edits.
- **AC-X3**: NO modifications to `tests/fixtures/playground/` or
  `tests/fixtures/playground-*/` (standing playground rule).
- **AC-X4**: NO commits or pushes without user authorization
  (standing rule).
- **AC-X5**: Mode A reports `Total = 136 + N`, `Pass = 136 + N`,
  `Fail = 0`, where N ∈ {0, 1, 2, 3, 4} is the count of
  F72-committed cohort rows. The exact N is documented in the
  expectation file.
- **AC-X6**: The expectation file at
  `.harness/expectations/regseq-driver-72.md` is authored with
  per-sub-task outcome records, path-choice rationale, and any
  escalation rationale.
