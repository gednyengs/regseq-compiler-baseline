# regseq-driver-72 expectations: F71 follow-up trio

Task: `.harness/tasks/regseq-driver/72-f71-followup-trio.md`
Plan: `.harness/audits/2026-05-23-f71-followup-trio-plan.md`

## Outcome summary

All three sub-tasks PASSED. Sidecar grew 136 -> 140 (N = 4, the
maximum). Mode A: `Total = 140`, `Pass = 140`, `Fail = 0`, exit 0.
B9 regression: the 136 pre-F72 rows have identical per-row PASS/FAIL
classification pre- vs post-change (verified by diffing the baseline
and final Mode A reports).

| Sub-task | Path chosen | Cohort rows | Disposition |
| -------- | ----------- | ----------- | ----------- |
| A | A.2.ii (sibling arm) | SK-004, SK-005 | PASS (+2) |
| B | B.2.a (lazy-allocate) | FF-007 | PASS (+1) |
| C | C.2.a.i (positive carve-out row) | RJ-009 | PASS (+1) |

## Sub-task A: SyntheticItemKind sub-variant predicate

**Path: A.2.ii (RECOMMENDED) -- sibling arm.** Added a new
`InspectionPredicate::SyntheticItemKindSub { kind: String, sub:
String }` variant adjacent to the untouched `SyntheticItemKind(String)`
arm, plus a `synthetic_kind_subvariant_tag` helper and a dispatch arm,
all in `crates/bridge-coverage-runner/src/inspect.rs`.

**Serde-trait verification (AC-A1).** The enum is adjacently tagged
(`#[serde(tag = "kind", content = "arg", rename_all = "PascalCase")]`).
The new struct variant serialises as
`{ kind = "SyntheticItemKindSub", arg = { kind = "...", sub = "..." } }`;
the existing `SyntheticItemKind(String)` arm continues to serialise as
`{ kind = "SyntheticItemKind", arg = "ClosureCall" }`. The two are
distinct top-level tags, so every legacy row deserialises unchanged.
No backward-compatibility shim is required (this is exactly why A.2.ii
is preferred over A.2.i). Verified empirically: the F71-committed
`ClosureCall` rows (IX-004, IX-010, BD-007, FT-011..014, SK-003,
IX-014) all remain green under Mode A, and the new SK-004/SK-005 rows
PASS via the sub-variant arm (AC-A3, A.2.ii branch).

**Snippets.** `SK-004.rs` (capture-mutating `FnMut` closure ->
`ClosureCall { call_kind: CallMut }`) and `SK-005.rs` (move `FnOnce`
closure -> `ClosureCall { call_kind: CallOnce }`). Both are fresh
files with their own entry functions (not copies of FT-013/FT-014/
IX-014), so the duplicate-snippet drift check stays clean. SK-004's
`sub = "CallMut"` predicate PASSes and SK-005's `sub = "CallOnce"`
predicate PASSes -- proving the new arm discriminates the sub-axis
(a CallOnce snippet would fail a CallMut predicate, per the
`synthetic_item_kind_sub_arm_distinguishes_closure_call_subvariants`
unit test).

## Sub-task B: ClosureOnceShim Phase-5 binding

**Path: B.2.a -- lazy-allocate via `lookup_or_create_direct_instance`.**

**Root cause (empirically reproduced).** The FF-007 Fn-as-FnOnce
coercion shape (`let c = |n: i32| n + 1; call_once(c, 5)` against
`call_once<F: FnOnce(i32) -> i32>`) makes rustc resolve
`<{closure} as FnOnce>::call_once` to an
`InstanceKind::ClosureOnceShim`. The Phase-2 callee pre-scan at
`crates/regseq-driver/src/bridge/mod.rs:1084-1093` interns the
`core::ops::function::FnOnce::call_once` trait-method Item (observed as
`ItemId(6)` for the FF-007 snippet). At Phase-5 body materialisation
the closure-call rewrite at
`crates/regseq-driver/src/bridge/body/terminator.rs` redirects the
actual Call terminator to the `ClosureCall { CallOnce }` synth
Instance, so the pre-interned `FnOnce::call_once` Item is left with
zero Instances. It is not source-local (so
`construct_function_instances` skips it) and not generic / not
signature-non-rigid, so it lands in bucket 1, which requires exactly
one Instance -- tripping `ItemMissingInstance { item: ItemId(6),
item_kind: Function }` at the Phase-5 completeness check.

Note: the spec's framing assumed a `ClosureOnceShim` Instance already
existed and its body referenced an unbound Item. The empirical reality
is that NO `ClosureOnceShim` Instance existed -- the orphan was the
pre-interned `FnOnce::call_once` Item with zero Instances. The fix
below produces the `ClosureOnceShim` Instance the FF-007 predicate
expects, which both binds the orphan and surfaces the flavor.

**Fix.** In the closure-call rewrite (`rewrite_closure_call_shim`),
after the `ClosureCall` Instance is resolved, call the new
`bind_closure_once_shim_instance` helper. When
`Instance::try_resolve` shows the call resolves to an
`InstanceKind::ClosureOnceShim`, the helper lazy-allocates a
`ClosureOnceShim`-flavored Function Instance for the `FnOnce::call_once`
Item. The actual Call terminator is unchanged (still the `ClosureCall`
Instance, so the closure-call rewrite path is NOT regressed -- AC-B4 /
B5). The shim Instance is uncalled and carries `body: None`; the
Phase-5 materialiser already treats `FunctionInstanceFlavor::
ClosureOnceShim` as a flavor with no own MIR
(`crates/regseq-driver/src/bridge/body/materialise.rs:246-269`).

**Why FnAbi::Rust (distinct from F70's FF-005 RustCall ABI).** The
shim sig is built manually as `FnAbi::Rust` (inputs = the closure-call
shim's by-value positional inputs, which equal the `CallOnce`
receiver-plus-untupled shape; output = the closure's own return type).
We deliberately do NOT route through `callsite_fn_sig` /
`translate_fn_sig` on `FnOnce::call_once`'s `extern "rust-call"`
signature, which would raise `UnsupportedAbi` -- that is F70's FF-005
territory and is explicitly out of scope. This keeps FF-007 a Phase-5
completeness fix, not an ABI-translation change.

**Cache-key invariant + no-duplicate verification (AC-B5 / B10).** The
lazy allocation keys the `function_instance_for_item` table on
`(ItemId(FnOnce::call_once), ArgsKey(model_args),
FunctionInstanceFlavor::ClosureOnceShim)`. This is the same
`(ItemId, ArgsKey, FunctionInstanceFlavor)` shape used by every other
`lookup_or_create_direct_instance` call site and by
`construct_closure_call_instances`, so repeated FF-007-style call
sites for the same shim converge on one Instance. Verification
(Mode A delta-check): the FF-007 input PASSes the bucket-1
completeness check, which reports `Bucket1MultipleInstances` if a
Function Item carries more than one Instance. Since completeness
passes, the `FnOnce::call_once` Item carries exactly one Instance --
the `ClosureOnceShim`. No duplicate is created.

**Invariant preservation (AC-B6 / B11 / B12).** `git diff` confirms NO
changes to `crates/regseq-model/src/model.rs` (bucket-1 validation at
`:3622-3627` untouched -- the fix binds the Item, it does not weaken
validation) and NO changes to
`crates/regseq-driver/src/bridge/ty.rs` (F70's
`AuthorizedProjectionFolder` untouched).

## Sub-task C: RJ-009 source-shape resolution

**Path: C.2.a.i -- positive-row coverage of the bucket-4 carve-out.**

**Bounded-gate disposition (B6).** C.2.b (surface a `NonRigidTy::Param`
rejection through the oracle) is INFEASIBLE. F71 tried 3 source shapes
(monomorphic `id::<u32>`, unused generic, generic trait method) -- all
`Ok`. F72's 4th probe was a non-generic `-> impl Fn(i32) -> i32` RPIT
function, which the bucket-4 carve-out absorbs (`Ok`, no rejection).
Code analysis explains why no Param rejection can surface through the
current oracle:

- Any `Param`-bearing signature makes the Item generic
  (`generics_of(def_id).count() > 0`), which the four-bucket validator
  routes to bucket 3 and skips at
  `crates/regseq-driver/src/bridge/mod.rs:1584` BEFORE the bucket-4
  carve-out's `translate_fn_sig`.
- Trait-method `Self` Params are skipped at
  `crates/regseq-driver/src/bridge/mod.rs:1600-1606`.
- The carve-out at `crates/regseq-driver/src/bridge/mod.rs:1644-1658`
  therefore only fires for non-generic, uncalled Items with a
  non-rigid signature, whose canonical shape is `Alias(Opaque)` (RPIT)
  -- never `Param`.

Per the bounded gate, having exhausted four shapes (3 from F71 + 1
here), the implementer proceeds to path C.2.a.

**Path within C.2.a: C.2.a.i (keep RJ-009 a positive row).** Added a
new `InspectionPredicate::SignatureNonRigid` arm that matches when any
Item carries `BridgeItemFlags.signature_non_rigid == true`. The RJ-009
snippet (`crates/bridge-coverage-runner/snippets/RJ-009.rs`) is a
private, uncalled `fn opaque_adder() -> impl Fn(i32) -> i32`: not a
mono-item root and never called, so the bridge creates no reactive
Instance; `construct_function_instances` translates its identity
signature, the RPIT `Alias(Opaque)` return raises
`BridgeError::NonRigidTy`, and the carve-out records
`signature_non_rigid = true`. RJ-009 PASSes.

**Serde sub-path (alpha / beta): NEITHER -- the spec premise is
incorrect.** The spec's C.2.a.i requires exposing
`signature_non_rigid` via serde (alpha: derive Serialize/Deserialize +
remove `#[serde(skip)]`; beta: add a serializable mirror) because it
states "the runner's deserialized model therefore sees an empty
item_bridge_flags map." That premise is FALSE for this runner. The
oracle runs the bridge IN-PROCESS and inspects the `RegseqModel` the
bridge returns directly (`crates/bridge-coverage-runner/src/oracle.rs`
-> `crates/test-ir-fixtures/src/lib.rs:470`,
`BridgeOutcome::Ok(model)`); there is no JSON/TOML round-trip on the
runner's positive-row path, so `item_bridge_flags` is fully populated.
The `SignatureNonRigid` predicate reads `model.item_bridge_flags`
directly. Adding serde would be dead code for a round-trip that does
not occur (CLAUDE.md 1.2: no speculative complexity). This is a
deliberate, empirically-verified deviation: RJ-009 PASSing under
Mode A with the predicate reading the in-process map is the proof that
the flag is visible without serde. Consequently NO edits were made to
`crates/regseq-model/src/model.rs`, `crates/regseq-model/src/item.rs`,
or the bridge carve-out at `crates/regseq-driver/src/bridge/mod.rs`.

**Semantic shift (documented per C.2.a.i).** RJ-009's matrix construct
is `NonRigidTyKind::Param`. It is reframed as "positive-row coverage of
the bucket-4 carve-out." The carve-out flag is kind-agnostic (a plain
`bool`; it does not record `Param` vs `Alias`), and -- per the
bounded-gate analysis above -- no realistic source shape surfaces a
`Param`-kind non-rigid type through the carve-out (only `Alias`). So
RJ-009 verifies the bucket-4 carve-out mechanism, not a `Param`
rejection specifically. The matrix-doc row and `rows.toml` comment
both record this shift.

**RJ-010..013 NOT committed (AC-C3 / AC-X1).** The `SignatureNonRigid`
predicate exists now, but RJ-010 (`Bound`) / RJ-011 (`Placeholder`) /
RJ-012 / RJ-013 are independent rows requiring their own snippets and
remain out of scope. No `task.py add-task` filings were made.

## Cross-cutting

- **AC-X2 (diff scope).** Changes confined to: `inspect.rs` (sub-tasks
  A + C predicates), `terminator.rs` (sub-task B fix), `rows.toml`
  (4 cohort rows), `snippets/SK-004.rs` `SK-005.rs` `FF-007.rs`
  `RJ-009.rs`, and `bridge-coverage-matrix.md` (4 per-row updates). No
  serde / model / item / bridge-mod edits (sub-task C took the
  in-process path). No `tests/fixtures/playground*` edits.
- **AC-X5.** N = 4; sidecar 140; Mode A `Total = Pass = 140`, `Fail = 0`.
- **No RustCall ABI re-attempt.** Sub-task B builds the shim sig as
  `FnAbi::Rust`; F70's FF-005 escalation stands untouched.
