# regseq-driver-72 plan: F71 follow-up trio

**Status**: APPROVED (2026-05-23). Plan was approved clean after three
Codex review rounds:
- Round 1 (session `019e55ec`): NOT_READY — 6 actionable findings
  (3 MAJOR, 3 MINOR).
- Round 2 (session `019e55f2`): APPROVED_WITH_MINOR_FIXES — 2 minor
  findings.
- Round 3 (session `019e55f4`): APPROVED clean.

The task spec at `.harness/tasks/regseq-driver/72-f71-followup-trio.md`
implements this plan.

**Scope**: bundle the three follow-up gaps surfaced by F71's escalation
records into a single implementer task. The bundle is justified because
the three sub-tasks are independent (no cross-dependency), small enough
that a single Codex review cycle is cheaper than three, and all share
the same triggering context (F71 cohort residuals).

**Bundled sub-tasks** (independent; each can PASS or escalate without
affecting the others):

- **Sub-task A** — runner-precision: add a `SyntheticItemKind`
  sub-variant matching path to the `InspectionPredicate` set so the
  `ClosureCall { call_kind }` axis can be distinguished by predicate.
  Unblocks SK-004 (CallMut) + SK-005 (CallOnce) for sidecar commit.
- **Sub-task B** — bridge fix: bind the ClosureOnceShim's underlying
  Function Item during synthesis pre-scan (or lazy-allocate it via
  `lookup_or_create_direct_instance`) so the Phase-5 body pass's
  bridge-completeness validation succeeds. Unblocks FF-007 for sidecar
  commit.
- **Sub-task C** — investigation + decision: resolve RJ-009's
  source-shape problem by either (a) exposing the bridge bucket-4
  carve-out flag (`signature_non_rigid: true`) to the runner's
  rejection oracle so RJ-009/010/011/012/013 have a surfacing path, OR
  (b) authoring a polymorphic-fn surface that bypasses the carve-out
  and surfaces `NonRigidTy::Param` to the existing rejection oracle.
  The spec must require the implementer to investigate AND choose
  before any code lands; both paths are admissible.

---

## 1. Background

F71 closed Category 1 (closure-family reattempt) but recorded four
escalations:

- **RJ-009** (empirical Mode 1, Tier 1): bucket-4 carve-out at
  `crates/regseq-driver/src/bridge/mod.rs:1644-1658` silently absorbs
  the polymorphic `translate_fn_sig` NonRigidTy error, so all three
  attempted source shapes failed to surface a Param rejection at HEAD.
  Documented at
  `.harness/expectations/regseq-driver-71.md:329-381`.
- **FF-007** (empirical Mode 1, Tier 2): NEW bridge defect distinct
  from F70's RustCall ABI rejection. Two source shapes (Fn-as-FnOnce
  coercion, FnMut-as-FnOnce coercion) hit the Phase-5
  `ItemMissingInstance` failure. Documented at
  `.harness/expectations/regseq-driver-71.md:383-433`.
- **SK-004** (design-time Mode 2, Tier 3): runner-precision gap at
  `crates/bridge-coverage-runner/src/inspect.rs:50-53` +
  `:168-174` + `:604-618`. The `synthetic_kind_variant_tag` function
  collapses all `ClosureCall { call_kind }` sub-variants to the single
  `"ClosureCall"` tag, so the SK-003 row's predicate would falsely
  cover SK-004 and SK-005 if they were committed. Documented at
  `.harness/expectations/regseq-driver-71.md:435-470`.
- **SK-005** (design-time Mode 2, Tier 3): same root cause as SK-004
  but for the `CallOnce` sub-variant. Documented at
  `.harness/expectations/regseq-driver-71.md:472-487`.

The F71 task spec's Non-Goals explicitly deferred all three classes
(no new `InspectionPredicate` arms; no bridge code changes; no source-
shape design beyond the cohort sweep). F72 picks them up.

---

## 2. Code-surface grounding

Verified at HEAD (2026-05-23):

- `crates/bridge-coverage-runner/src/inspect.rs:50-51` — current
  `InspectionPredicate::SyntheticItemKind(String)` single-positional
  arm. Sub-task A extends this arm.
- `crates/bridge-coverage-runner/src/inspect.rs:168-182` —
  `synthetic_kind_variant_tag` collapses `ClosureCall { call_kind: ... }`
  to `"ClosureCall"`. Sub-task A adds a sub-variant lookup adjacent
  to this.
- `crates/bridge-coverage-runner/src/inspect.rs:604-618` — dispatch
  body matches only on the tag. Sub-task A extends the matching arm
  to include the sub-variant.
- `crates/regseq-driver/src/bridge/body/terminator.rs:209-214` —
  `LangItem::Fn` / `FnMut` / `FnOnce` mapping to `ClosureCallKind`
  produced inside `synthesise_closure_call_shim`'s rewrite. Reference
  for sub-task B's understanding of where shim materialization
  diverges between `ClosureCall` (rewritten) and `ClosureOnceShim`
  (not rewritten).
- `crates/regseq-driver/src/bridge/body/terminator.rs:340-371` —
  `lookup_or_create_direct_instance` precedent used by the closure-
  call shim path. Sub-task B's path (a) lazy-allocate would mirror
  this pattern.
- `crates/regseq-driver/src/bridge/mod.rs:1644-1658` — bucket-4
  carve-out. Captures `BridgeError::NonRigidTy { .. }`, sets
  `BridgeItemFlags { signature_non_rigid: true, .. }`, pushes no
  Instance. Sub-task C's path (a) routes this flag to the runner
  rejection oracle.

---

## 3. Sub-task A: `SyntheticItemKind` sub-variant predicate

### A.1 Problem

`InspectionPredicate::SyntheticItemKind(String)` matches only the
variant tag returned by `synthetic_kind_variant_tag`, which is one of
`{"None", "ClosureCaptures", "ClosureCall", "ClosureDropGlue",
"DropGlue", "VTableType", "VTableInstance", "VTableMethodShim",
"VTableDropShim", "ConstMemoryStatic"}`. The `ClosureCall` tag is
emitted regardless of `call_kind` (`Call` / `CallMut` / `CallOnce`),
so SK-003 / SK-004 / SK-005 collide.

### A.2 Proposed scope

Extend the runner's predicate surface so the `ClosureCall { call_kind }`
axis can be discriminated. Three plausible shapes; the spec
**recommends A.2.ii** as default because of a hard serde constraint
explained below. The implementer may select A.2.i if (and only if)
they also implement the backward-compatibility parser noted in A.2.i.

- **Shape A.2.i** — change the arm from `SyntheticItemKind(String)`
  to `SyntheticItemKind { kind: String, sub: Option<String> }`.
  **Serde constraint**: the enum at
  `crates/bridge-coverage-runner/src/inspect.rs:41-43` is adjacently
  tagged with `#[serde(tag = "kind", content = "arg", rename_all =
  "PascalCase")]`, and existing TOML rows use a string `arg`, e.g.
  `crates/bridge-coverage-runner/matrix_metadata/rows.toml:1387` +
  `:1457`. Re-shaping the variant to a struct would normally force
  `arg = { kind = "...", sub = "..." }`, which breaks every existing
  row that uses `arg = "ClosureCall"`. To preserve backward
  compatibility, A.2.i REQUIRES a custom deserializer (e.g.,
  `#[serde(untagged)]` shim that accepts either the string form or
  the struct form) and a sidecar-load regression check.
- **Shape A.2.ii** *(RECOMMENDED)* — keep the existing
  `SyntheticItemKind(String)` arm untouched and add a sibling arm,
  e.g., `SyntheticItemKindSub { kind: String, sub: String }`.
  Larger predicate surface but no backward-compatibility risk: every
  existing row continues to deserialize unchanged. SK-004 / SK-005
  use the new arm; SK-003 (and any other `ClosureCall` rows) keep
  the old arm.
- **Shape A.2.iii** — encode `"ClosureCall/CallMut"` into the
  existing `String` via a delimiter. Smallest code change but
  couples encoding into matrix-row TOML data, which is a
  maintainability hazard. **Not recommended.**

**Spec authority**: A.2.ii is the spec's default recommendation. If
the implementer chooses A.2.i, they must (a) implement the
backward-compat parser shim, (b) extend AC-A3 to include a full
sidecar-load regression check (every existing `SyntheticItemKind`
row continues to parse and inspect green), and (c) document the
deserializer design in their expectation file. A.2.iii is disallowed
absent explicit justification.

### A.3 Dispatch + tag updates

When sub-variant matching is active, the dispatch must:

1. Match the parent tag via the existing `synthetic_kind_variant_tag`
   function.
2. If the requested `sub` is `Some(s)`, additionally interrogate the
   matched entry's inner enum for the named sub-variant.

This requires a new helper (e.g., `synthetic_kind_subvariant_tag`)
that returns:
- `Some("Call" | "CallMut" | "CallOnce")` for `ClosureCall { call_kind }`
- `None` for variants without a meaningful sub-axis

### A.4 Sidecar additions

After the predicate lands, the implementer commits SK-004 and SK-005
rows to the sidecar:

- **SK-004**: snippet at `crates/bridge-coverage-runner/snippets/SK-004.rs`
  matching the FT-013 source shape (`|x| total += x` capture-mutating
  closure → `LangItem::FnMut` → `ClosureCallKind::CallMut`).
  Predicate uses `kind = "ClosureCall", sub = "CallMut"`.
- **SK-005**: snippet at `crates/bridge-coverage-runner/snippets/SK-005.rs`
  matching the IX-014 / FT-014 source shape (move-FnOnce capture →
  `LangItem::FnOnce` → `ClosureCallKind::CallOnce`).
  Predicate uses `kind = "ClosureCall", sub = "CallOnce"`.

### A.5 Acceptance criteria

- AC-A1: a new sub-variant axis is reachable from the runner's
  `InspectionPredicate` set; the predicate-shape decision is
  documented in the implementer's expectation file.
- AC-A2: SK-004 and SK-005 rows committed to sidecar with the new
  predicate; both PASS under Mode A green.
- AC-A3: regression check. Sufficiency depends on the predicate-
  shape choice:
  - If **A.2.ii** (recommended): the FT-013 snippet's
    `SyntheticItemKind=ClosureCall` row remains green. No further
    sidecar-load coverage required because the existing arm is
    untouched.
  - If **A.2.i**: ALL existing `SyntheticItemKind` sidecar rows
    (including but not limited to the rows at
    `crates/bridge-coverage-runner/matrix_metadata/rows.toml:1387`
    + `:1457`, plus the FT-013 closure-call row) must deserialize
    and inspect green under the backward-compat parser shim. A full
    sidecar-load regression sweep is required.
  - If **A.2.iii** (disallowed absent explicit justification): not
    applicable.

---

## 4. Sub-task B: ClosureOnceShim Phase-5 binding

### B.1 Problem

The Fn-as-FnOnce coercion shape:

```rust
let c = |n: i32| n + 1;
call_once(c, 5);
```

against `call_once<F: FnOnce(i32) -> i32>(f: F, x: i32) -> i32`
triggers rustc to synthesize an `InstanceKind::ClosureOnceShim` at
monomorphization. The bridge maps this through
`function_instance_flavor_for_kind` to
`FunctionInstanceFlavor::ClosureOnceShim`. Phase-5 body pass's bridge-
completeness validation then fails with:

```
ItemMissingInstance { item: ItemId(N), item_kind: Function }
```

where `ItemId(N)` is the underlying Item that the shim's body
references. The closure-call shim (F62) handles its parallel case via
`lookup_or_create_direct_instance` at
`crates/regseq-driver/src/bridge/body/terminator.rs:354-369`; the
ClosureOnceShim path does not.

### B.2 Proposed scope

Two admissible implementation paths:

- **Path B.2.a** — lazy-allocate the Function Item the shim references
  through `lookup_or_create_direct_instance`, mirroring F62's
  closure-call shim pattern at
  `crates/regseq-driver/src/bridge/body/terminator.rs:354-369` and
  the analogous call-site path at
  `crates/regseq-driver/src/bridge/body/terminator.rs:559-566`.
  Implementer must verify the cache-key shape matches whatever
  eager-construction the rest of the bridge uses (or else create a
  duplicate Instance).
- **Path B.2.b** — bind the ClosureOnceShim's underlying Item during
  the synthesis pre-scan, so by the time Phase-5 validation runs, the
  Item is already in the model.

**No third path**: weakening or special-casing Phase-5 bridge-
completeness validation to tolerate the missing Item/Instance is NOT
admissible. Bucket 1 validation requires exactly one instance and
reports `ItemMissingInstance` when count is zero at
`crates/regseq-model/src/model.rs:3622-3627`; relaxing that
invariant for ClosureOnceShim would break the bridge-completeness
contract that the rest of the materialization surface depends on.
The fix must preserve instance completeness, not loosen it.

The implementer must choose between paths (a) and (b) based on
which is more spec-faithful per `spec.md` §9.3-9.4 instance/flavor
identity (`docs/discussions/regseq-model/spec.md:2525-2710`),
closure-call synthesis (`docs/discussions/regseq-model/spec.md:4579-4614`),
and Phase-5 body materialization behaviour at
`crates/regseq-driver/src/bridge/body/materialise.rs:335-389`. The
F62 closure-call shim precedent at
`crates/regseq-driver/src/bridge/body/terminator.rs:354-369`
provides the concrete pattern for path (a). The decision must be
documented in the implementer's expectation file.

### B.3 Sidecar addition

After the fix lands, the implementer commits FF-007 to the sidecar:

- **FF-007**: snippet at `crates/bridge-coverage-runner/snippets/FF-007.rs`
  matching the Fn-as-FnOnce coercion shape (the cleanest of the three
  candidates documented in F71's expectation).
  Predicate uses `FunctionInstanceFlavor=ClosureOnceShim`.

### B.4 Acceptance criteria

- AC-B1: the Phase-5 `ItemMissingInstance` failure no longer fires
  for the FF-007 Fn-as-FnOnce coercion source shape.
- AC-B2: FF-007 row committed to sidecar; PASS under Mode A green.
- AC-B3: matrix row at `docs/discussions/regseq-model/bridge-coverage-matrix.md`
  for FF-007 is refreshed from `blocked-bridge-itemmissinginstance`
  to `committed` (or equivalent post-F71 status).
- AC-B4: closure-call shim rows (BD-007, FT-011-014, IX-004, IX-010,
  IX-014, SK-003) remain green (regression check).
- AC-B5: if path B.2.a is taken, the implementer documents which
  cache-key invariants are preserved and how the eager-construction
  duplicate-Instance scenario is prevented.

---

## 5. Sub-task C: RJ-009 source-shape resolution

### C.1 Problem

RJ-009's matrix row predicate requires a `NonRigidTy::Param` rejection
to be observable from a Rust source shape. F71's 3-retry sweep
demonstrated that the bridge's bucket-4 carve-out at
`crates/regseq-driver/src/bridge/mod.rs:1644-1658` silently absorbs
the polymorphic `Function` item's `translate_fn_sig` error, setting
`BridgeItemFlags { signature_non_rigid: true, .. }` and pushing no
Instance. The bridge therefore does NOT surface a `NonRigidTy::Param`
rejection from any F71-cohort source shape; RJ-009/010/011/012/013
have no surfacing path through the current rejection oracle.

### C.2 Proposed scope

The implementer MUST investigate and choose ONE of:

- **Path C.2.a** — runner-side oracle extension: the bridge's bucket-
  4 carve-out already records `signature_non_rigid: true` on
  `BridgeItemFlags`. The runner currently does not consume this flag.
  However, this path is NOT just a one-line predicate addition — it
  requires an explicit decision about the runner's outcome model:

  - The runner's positive-row inspection only runs against
    `OracleOutcome::Ok(model)` at
    `crates/bridge-coverage-runner/src/main.rs:191-197`; rejected
    rows match against `BridgeError` at
    `crates/bridge-coverage-runner/src/main.rs:207-220`; a rejected
    row that produced `Ok` fails outright at
    `crates/bridge-coverage-runner/src/main.rs:223-230`.
  - The `BridgeItemFlags.signature_non_rigid` flag is bridge-only
    and currently `#[serde(skip)]` at
    `crates/regseq-model/src/model.rs:85-94`; bucket-4 validation
    accepts zero-instance Function Items at
    `crates/regseq-model/src/model.rs:3612-3620`. So bucket-4 rows
    today produce `OracleOutcome::Ok(model)`, not a rejection.

  This means the implementer must explicitly choose **one** of:

  - **C.2.a.i** — keep RJ-009 as a positive row. Expose the bucket-
    4 flag via serde, add a new `InspectionPredicate` arm (e.g.,
    `SignatureNonRigid` or `BridgeItemFlag { name, expect_true }`)
    that interrogates `BridgeItemFlags.signature_non_rigid`. The
    matrix-row TOML's existing "rejected/uncovered" semantic for
    RJ-009 must be reframed as "positive-row coverage of the
    bucket-4 carve-out". The implementer's expectation file must
    document this semantic shift.
  - **C.2.a.ii** — introduce a third outcome class beyond
    Ok/BridgeError (e.g., `OracleOutcome::CarveOut(model, flags)`).
    Adds an enum variant to the runner's outcome model and a new
    expectation matcher for the rejection-like signal. Larger
    surface than C.2.a.i; only justifiable if the carve-out signal
    is expected to be re-used across many future rows.

- **Path C.2.b** — source-shape engineering: find or construct a
  polymorphic-function source shape that bypasses the bucket-4
  carve-out (e.g., a polymorphic trait method whose receiver type is
  not fully bound at the monomorphization site, or a polymorphic
  generic that survives `Instance::try_resolve`). Commit RJ-009 with
  this shape against the existing rejection oracle.

**Bounded decision gate**: the implementer attempts up to four
candidate source shapes for path C.2.b (extending F71's three-retry
sweep by one shape). If none surface a `NonRigidTy::Param` rejection
through the current oracle:

- The implementer either proceeds to path C.2.a (committing to
  C.2.a.i or C.2.a.ii in their expectation file), OR
- The implementer escalates sub-task C independently with rationale;
  sub-tasks A and B are NOT held by C's escalation. Their sidecar
  additions land regardless.

Either path is acceptable. The implementer must document which path
was taken and why, including the bounded-gate disposition.

### C.3 Sidecar addition

After the chosen path lands:

- If **C.2.a**: a new `InspectionPredicate` arm (e.g.,
  `SignatureNonRigid`) plus a snippet at
  `crates/bridge-coverage-runner/snippets/RJ-009.rs` carrying a generic-
  fn surface that triggers the bucket-4 carve-out. RJ-009 PASSes.
- If **C.2.b**: snippet at `crates/bridge-coverage-runner/snippets/RJ-009.rs`
  carrying the engineered source shape. The existing rejection-oracle
  predicate continues to work; RJ-009 PASSes.

### C.4 Acceptance criteria

- AC-C1: RJ-009 row committed to sidecar; PASS under Mode A green.
  **OR** sub-task C escalates per the bounded decision gate in C.2,
  with rationale documented in the expectation file. Sub-task C
  escalation does NOT block AC-A* or AC-B*.
- AC-C2: implementer's expectation file documents which path was
  taken (C.2.a.i, C.2.a.ii, or C.2.b), the rationale, and the
  bounded-gate disposition (the four candidate shapes tried for
  C.2.b, if any).
- AC-C3: if path C.2.a is taken: RJ-010/011/012/013 are NOT
  automatically committed — those are independent rows requiring
  their own snippets even though the predicate now exists.
- AC-C4: matrix row at `docs/discussions/regseq-model/bridge-coverage-matrix.md`
  for RJ-009 is refreshed from `partial` (or current status) to
  `committed` (or `escalated-c-bounded-gate` if sub-task C escalates).

---

## 6. Order of execution (implementer guidance)

The three sub-tasks have no hard dependencies. Recommended order
inside the single F72 task:

1. **Sub-task A first** — smallest, well-isolated runner change. Once
   the predicate exists, the implementer has the framework for any
   sub-axis matching that sub-task C path (C.2.a) might need.
2. **Sub-task B second** — bridge fix with two implementation paths.
   The implementer's path choice (a vs b) must be documented before
   coding.
3. **Sub-task C last** — investigation-first. If sub-task A added a
   sub-variant matching mechanism, sub-task C path (a) inherits the
   shape conventions; if not, sub-task C still functions independently
   on its own predicate.

If any sub-task escalates (Mode 1 or Mode 2), the others proceed
unaffected.

---

## 7. Out-of-scope

The following are explicitly NOT in scope for F72:

- F68 audit slate items (B1, B2, C1, C2, F1). Those remain in their
  own future task slots per the audit ordering.
- Gap inventory Categories 2 (FF-002 audit), 3 (F7 discovery batch),
  4 (Section 7 validator audit), 5 (F5 automation), 6 (matrix-
  precision pass).
- Refactoring or "improving" any of the touched files beyond the
  surgical changes each sub-task requires.
- New matrix rows. F72 only touches rows already in the matrix +
  sidecar (SK-004, SK-005, FF-007, RJ-009).
- RJ-010 / RJ-011 / RJ-012 / RJ-013 row commits. These are explicitly
  out-of-scope even if sub-task C path (a) makes them tractable.
- Snippet authoring beyond the 4 rows (SK-004, SK-005, FF-007, RJ-009).
- Any weakening or special-casing of Phase-5 bridge-completeness
  validation. Bucket 1 reports `ItemMissingInstance` for zero-
  instance Function Items at
  `crates/regseq-model/src/model.rs:3622-3627`; the F72 fix must
  preserve that invariant, not loosen it.
- Any change to F70's `AuthorizedProjectionFolder` at
  `crates/regseq-driver/src/bridge/ty.rs:81-143` (folder
  implementation), the F70 normalization-prelude call at
  `crates/regseq-driver/src/bridge/ty.rs:55-69` inside
  `translate_erased_ty`, or the downstream `TyKind::Alias` rejection
  arm at `crates/regseq-driver/src/bridge/ty.rs:426`. C.2.b source-
  shape probes must not silently depend on or widen F70's authorized
  projection set; if a probe requires broader normalization, that
  belongs in a separate task.

---

## 8. Risks + coordination

**Risk R-1**: sub-task B's implementation path (a) may produce
duplicate Instances if the cache-key shape doesn't match the eager-
construction surface. The closure-call shim (F62) hit and resolved
this exact issue; the implementer must verify the analogous invariant
for ClosureOnceShim. *Mitigation*: AC-B5 makes this explicit.

**Risk R-2**: sub-task C path (a) requires a new `InspectionPredicate`
arm (`SignatureNonRigid` or similar). If sub-task A is in flight at
the same time, the implementer must avoid two simultaneous predicate-
arm additions colliding. *Mitigation*: order-of-execution
recommendation places sub-task A first; sub-task C can pattern-match
the predicate-arm-addition shape sub-task A established.

**Risk R-3**: SK-003's currently-committed row at sidecar uses the
existing `SyntheticItemKind(String)` arm with `arg = "ClosureCall"`.
If sub-task A chooses A.2.i (struct re-shape), every existing
`SyntheticItemKind` row in `rows.toml` (verified examples:
`crates/bridge-coverage-runner/matrix_metadata/rows.toml:1387` +
`:1457`) would fail to deserialize without a backward-compat parser.
*Mitigation*: A.2.ii (the recommended path) sidesteps this entirely
by keeping the existing arm untouched; if the implementer chooses
A.2.i they must implement the backward-compat shim and AC-A3 must
include a full sidecar-load regression check.

**Risk R-4**: FF-007's matrix-row status was introduced by F71 as
`blocked-bridge-itemmissinginstance`. If F72's sub-task B refreshes
this status to `committed`, no other matrix row currently uses the
`blocked-bridge-itemmissinginstance` status class — the class becomes
unused. The matrix-precision audit (Category 6) tracks this; F72 does
NOT remove the status class even if FF-007 vacates it. *Mitigation*:
this is noted but no action required.

**Risk R-5**: F70 added `AuthorizedProjectionFolder` (implementation
at `crates/regseq-driver/src/bridge/ty.rs:81-143`) plus a node-local
normalization-prelude call at
`crates/regseq-driver/src/bridge/ty.rs:55-69` inside
`translate_erased_ty`, executed BEFORE the downstream `TyKind::Alias`
rejection arm at `crates/regseq-driver/src/bridge/ty.rs:426`. Sub-
task C path (b) source-shape probes that incidentally involve
`Pointee::Metadata` or `Fn`-family `::Output` projections would
benefit from F70's normalization. A C.2.b probe that fails because
of an *unauthorized* projection (e.g., `dyn Trait` metadata, or a
non-`Fn`-family alias) MUST NOT be "fixed" by widening
`AuthorizedProjectionFolder`'s authorized set; that belongs in a
separate task. *Mitigation*: out-of-scope list explicitly excludes
changes to `AuthorizedProjectionFolder` and the surrounding F70
surface.

---

## 9. Open questions for Codex review

Codex should specifically evaluate:

- **Q1**: is the bundle granularity correct? Should any sub-task be
  split out into its own F-numbered task, or is the bundle defensible?
- **Q2**: are the code-surface citations accurate at HEAD? (Spot-check
  any I missed.)
- **Q3**: for sub-task A, which shape (A.2.i, A.2.ii, A.2.iii) is
  the spec-best default? Should the spec leave the choice open or
  recommend one?
- **Q4**: for sub-task B, are paths (a) and (b) the only admissible
  options? Is there a third (e.g., changing how Phase-5 validation
  treats shim Instances)?
- **Q5**: for sub-task C, is the "investigate-then-choose" framing
  defensible, or does the spec need to pre-commit to one of (a) or
  (b)?
- **Q6**: is the order-of-execution recommendation correct, or should
  sub-task B precede sub-task A?
- **Q7**: out-of-scope list — are the F71-cohort rows
  RJ-010/011/012/013 correctly excluded? Should they be included if
  sub-task C path (a) is chosen?
- **Q8**: risk analysis — are R-1 through R-4 the right set, or is
  there a missing risk class (e.g., interaction with F70's
  AuthorizedProjectionFolder)?
- **Q9**: matrix-doc refresh scope — should the F72 spec require any
  prose updates to `bridge-coverage-matrix.md` beyond the per-row
  status refreshes (AC-B3, AC-C4)?
- **Q10**: is there a clean escalation framework if one sub-task fails
  partway through? The F71 spec used Mode 1 / Mode 2; should F72
  adopt the same vocabulary?
