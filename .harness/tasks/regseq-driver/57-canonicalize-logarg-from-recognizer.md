# regseq-driver-57: Canonicalize `<LogArg as From<T>>::from` recognizer

## Description

Close the dual-shape defensive-fallback surface left behind by
regseq-driver-56 in the diag-log `<LogArg as From<T>>::from` Call-callee
recognizer at
`crates/regseq-driver/src/bridge/diag_log/mod.rs::is_from_logarg_instance`
(`diag_log/mod.rs:39-91`). The recognizer currently matches TWO Instance
shapes:

- **Path 1 (`diag_log/mod.rs:55-61`)**: legacy trait-method shape -- the
  Item's rendered def_path ends with `::From::from`, and
  `instance.generic_args[0]` (Self) resolves to a `LogArg`-named ADT.
- **Path 2 (`diag_log/mod.rs:63-90`)**: post-regseq-driver-56 canonical
  impl-method shape -- the Item's def_path has a terminal
  `DefPathData::ValueNs("from")` under a `DefPathData::Impl` ancestor,
  and the Function Instance's `FnSig.output` resolves to a `LogArg`-named
  ADT.

Per spec.md §8.1 (`spec.md:2348-2350`, "Trait declarations are erased;
static dispatch resolves directly to the implementing function's
Instance") and the regseq-driver-56 amendment that follows it at
`spec.md:2354-2394` ("at a direct call site ... resolves
`ty::Instance::try_resolve(.., fully_monomorphized, def_id,
generic_args)` BEFORE interning the callee Item ... `InstanceKind::Item`
keys on the resolved impl-method DefId"), the bridge no longer interns
trait-DECLARATION DefIds as Function Items at static-dispatch call sites.
The recognizer's own doc comment at `diag_log/mod.rs:18-37` labels Path 1
as "pre-regseq-driver-56 / synthetic test fixtures" -- explicitly NOT
current bridge output.

Codex independently adjudicated this (verdict REMOVE) by tracing every
consumer of Path 1 to three in-tree unit-test builders that hand-construct
trait-method-shaped Instances WITHOUT going through the bridge:

1. `crates/regseq-driver/src/bridge/diag_log/peel.rs:1400-1448`
   (`make_from_trait_method_item`) used by
   `peels_logarg_from_wrapper_via_structural_recognition`
   (`peel.rs:1486`).
2. `crates/regseq-driver/src/bridge/diag_log/dead_item_elim.rs:1072-1120`
   (`push_from_trait_method_item`) used by
   `eliminate_residue_removes_wrapper_seed_and_chained_logarg_artifacts`
   (`dead_item_elim.rs:1130`).
3. `crates/regseq-driver/src/bridge/diag_log/dead_item_elim.rs:1293-1300`
   (inline rendered-string overrides on items created via `push_item`)
   used by `eliminate_residue_marks_every_logarg_from_impl_as_dead`
   (`dead_item_elim.rs:1266`).

Each builder constructs a 4-segment trait-method def_path
(`[TypeNs(core), TypeNs(convert), TypeNs(From), ValueNs(from)]` for the
first two, or post-hoc rendered-string mutation `"foo::From::from"` /
`"bar::From::from"` for the third) so the Path-1 substring check matches.

Per the binding rule `feedback_no_defensive_fallbacks` ("Don't add shadow
fallbacks in the compiler to guard against rustc/toolchain regressions;
the toolchain is pinned"), Path 1 is a defensive shadow path in
production code that maintains compatibility with three obsolete
in-memory synthetic-test shapes. The real-bridge integration test at
`crates/regseq-driver/tests/bridge_diag_log_dead_item_elim.rs:431` already
exercises the canonical impl-method shape post-regseq-driver-56 and is
the normative coverage; Path 1 is purely fixture-compatibility scaffolding.

After this task lands:

- `is_from_logarg_instance` matches Path 2 ONLY.
- The three test fixture builders construct canonical impl-method-shaped
  Items (`DefPathData::Impl` ancestor + terminal `DefPathData::ValueNs("from")`
  + `FnSig.output = logarg_ty` on every Function Instance).
- The doc comment at `diag_log/mod.rs:18-37` drops the dual-shape framing
  and describes Path 2 as the sole canonical recognizer.
- No production-path consumer references Path 1.
- All three affected synthetic tests + the real-bridge integration test
  remain green.

This task is purely a cleanup of the regseq-driver-56 implementer's
defensive scaffolding; no spec amendment is needed (the canonical shape
was already made normative by the regseq-driver-56 §8.1 amendment).

## Input Context

- `docs/discussions/regseq-model/spec.md` §8.1 impl-lifting rules
  (`spec.md:2341-2393`) -- normative call-site canonicalization contract
  (the regseq-driver-56 amendment at `spec.md:2354-2394` is the
  governing rule).
- `crates/regseq-driver/src/bridge/diag_log/mod.rs::is_from_logarg_instance`
  (`diag_log/mod.rs:39-91`) -- the current dual-shape recognizer; doc
  comment at `:18-37` labels Path 1 as "pre-regseq-driver-56 / synthetic
  test fixtures".
- `crates/regseq-driver/src/bridge/diag_log/peel.rs:1400-1448`
  (`make_from_trait_method_item`) -- Test fixture 1 builder.
- `crates/regseq-driver/src/bridge/diag_log/peel.rs:1486-1640`
  (`peels_logarg_from_wrapper_via_structural_recognition`) -- Test
  fixture 1 caller; constructs a wrapper Instance whose `FnSig.output =
  logarg_ty` is ALREADY set at `peel.rs:1512-1517`.
- `crates/regseq-driver/src/bridge/diag_log/dead_item_elim.rs:1072-1120`
  (`push_from_trait_method_item`) -- Test fixture 2 builder.
- `crates/regseq-driver/src/bridge/diag_log/dead_item_elim.rs:1130-1237`
  (`eliminate_residue_removes_wrapper_seed_and_chained_logarg_artifacts`)
  -- Test fixture 2 caller; constructs a wrapper Instance whose
  `FnSig.output = logarg_ty` is ALREADY set at `dead_item_elim.rs:1149-1155`.
- `crates/regseq-driver/src/bridge/diag_log/dead_item_elim.rs:1266-1397`
  (`eliminate_residue_marks_every_logarg_from_impl_as_dead`) -- Test
  fixture 3 caller; uses `push_item("from_u32_impl", ItemKind::Function)`
  + post-hoc `def_path.rendered` override at `:1293-1300`. The two
  Function Instances ALREADY have `FnSig.output = logarg_ty` set at
  `:1302-1308` and `:1323-1329`.
- `crates/regseq-driver/src/bridge/diag_log/dead_item_elim.rs:1025-1068`
  (`push_item`) -- the helper that test fixture 3 currently uses;
  produces a 2-segment def_path `[CrateRoot, TypeNs(leaf)]` rendered as
  `"dead_item_elim_unit_test::{leaf}"`. Insufficient for canonical
  impl-method shape; test fixture 3 needs either a new helper or a
  parameterized form.
- `crates/regseq-driver/tests/bridge_diag_log_dead_item_elim.rs:392-475`
  (`non_logarg_from_impl_in_unrelated_fixture_is_preserved`) -- real-bridge
  integration test that already asserts the canonical impl-method shape
  per the regseq-driver-56 commit's diff. UNCHANGED by this task; serves
  as the normative coverage that survives the Path 1 removal.
- `feedback_no_defensive_fallbacks` (user auto-memory) -- the binding
  rule that grounds the REMOVE verdict.

## Implementation Notes

### Change A: remove Path 1 from `is_from_logarg_instance`

**Anchor:** `crates/regseq-driver/src/bridge/diag_log/mod.rs:39-91`.

Delete the Path 1 block at `:55-61` entirely. The function body becomes
the current Path 2 logic only (`:63-90`), with the leading bounds checks
(`:42-53`) preserved.

Update the doc comment at `:18-37` to:

- Drop the "Path 1 (trait-method shape)" and "Path 2 (impl-method shape)"
  enumeration.
- Describe a single canonical recognizer: terminal `DefPathData::ValueNs("from")`
  segment, some ancestor `DefPathData::Impl` segment, function signature's
  output resolves to a `LogArg`-named nominal ADT.
- Cite the regseq-driver-56 amendment (`spec.md:2354-2394`) as the source
  of the canonical contract.

### Change B: canonicalize `peel.rs::make_from_trait_method_item`

**Anchor:** `crates/regseq-driver/src/bridge/diag_log/peel.rs:1400-1448`.

Rename to `make_from_impl_method_item` (or keep the same name; the
function's purpose is to build a `<LogArg as From<T>>::from` Item, which
is what the renamed function still does -- just with the post-fix
canonical shape). Replace the segments at `peel.rs:1410-1427` with:

```rust
segments: vec![
    DefPathSegment { data: DefPathData::TypeNs(core_seg), disambiguator: 0 },
    DefPathSegment { data: DefPathData::TypeNs(convert_seg), disambiguator: 0 },
    DefPathSegment { data: DefPathData::Impl, disambiguator: 0 },
    DefPathSegment { data: DefPathData::ValueNs(from_method), disambiguator: 0 },
],
```

Replace the rendered string at `peel.rs:1402` with one that reflects the
canonical impl-method form (e.g.,
`"core::convert::{impl#0}::from"`) so any test that inspects the rendered
string sees the canonical shape.

Update the doc comment at `peel.rs:1393-1399` to describe the new
canonical form and remove the trait-method-shape framing.

### Change C: canonicalize `dead_item_elim.rs::push_from_trait_method_item`

**Anchor:** `crates/regseq-driver/src/bridge/diag_log/dead_item_elim.rs:1072-1120`.

Same shape as Change B: rename / re-shape the segments + rendered string.
Update the doc comment at `dead_item_elim.rs:1070-1071`.

### Change D: canonicalize `eliminate_residue_marks_every_logarg_from_impl_as_dead`

**Anchor:** `crates/regseq-driver/src/bridge/diag_log/dead_item_elim.rs:1266-1397`.

The current test uses `push_item("from_u32_impl", ItemKind::Function)`
+ post-hoc `def_path.rendered` overrides at `:1293-1300`. Replace with a
new helper `push_from_impl_method_item(model, parent_seg: &str)` that
returns an Item with canonical impl-method def_path segments
(`[TypeNs(parent_seg), Impl, ValueNs("from")]`) and a rendered string
like `"{parent_seg}::{impl#0}::from"`.

Update the assertion lines at `:1357-1364` to compare against the new
rendered strings (e.g., `"foo::{impl#0}::from"`, `"bar::{impl#0}::from"`).

The two Function Instances' `FnSig.output = logarg_ty` already set at
`:1302-1308` and `:1323-1329` satisfy Path 2's signature check; no other
changes needed.

### Change E: doc comment hygiene

**Anchor:** `crates/regseq-driver/src/bridge/diag_log/mod.rs:18-37`,
plus the doc comments at `peel.rs:1393-1399`, `peel.rs:1478-1484`,
`dead_item_elim.rs:1070-1071`, plus the inline comment at
`dead_item_elim.rs:1294-1295`. Drop
all references to the "pre-regseq-driver-56" / "trait-method shape" /
"brittle pre-fix heuristic" framings; the canonical impl-method shape
is the sole recognizer post-removal.

## Non-Goals

- Do not modify `crates/regseq-driver/tests/bridge_diag_log_dead_item_elim.rs`.
  That real-bridge integration test already exercises the canonical
  impl-method shape per the regseq-driver-56 commit's diff
  (`bridge_diag_log_dead_item_elim.rs:431-475`); it is the normative
  coverage and stays untouched.
- Do not change the behavior of `eliminate_diag_log_peel_residue`,
  `peel_log_arg_from_wrapper`, or any pass that consumes
  `is_from_logarg_instance`. The recognizer's CONTRACT is unchanged
  (returns `true` for `<LogArg as From<T>>::from` Function Instances);
  only the input-shape it accepts narrows.
- Do not address the eager-enumeration asymmetry surfaced by manual
  playground stress-testing (top-level structs/fns absent vs.
  consts/statics surfaced). That is a separate spec dialogue; this task
  is exclusively the diag-log recognizer cleanup.
- Do not touch the spec.md §8.1 amendment from regseq-driver-56 (the
  canonical shape is already normative; no new amendment needed).
- Do not refactor `push_item` (`dead_item_elim.rs:1025`); leave it
  intact for the `LogArg` ADT Item construction and add a new
  `push_from_impl_method_item` helper for the From-impl Items.

## Test Plan

All tests live in `crates/regseq-driver/src/bridge/diag_log/` (inline
unit tests) or `crates/regseq-driver/tests/` (integration tests).
Before-fix expectation: B1-B3 fail Path 2 because the fixture builders
produce trait-method shapes (Path 1 currently masks this). After-fix
expectation: B1-B5 all pass against Path 2 only.

### B1: `peels_logarg_from_wrapper_via_structural_recognition` survives Path 1 removal

**Anchor:** `crates/regseq-driver/src/bridge/diag_log/peel.rs:1486-1640`.

The test currently passes because Path 1 matches its trait-method-shaped
Item. After Change A removes Path 1 and Change B canonicalizes the
builder, the test must still pass via Path 2. Verify:

- The wrapper Instance is still recognized by `is_from_logarg_instance`.
- The peel pass still collapses block 0's Call to a `Goto(block 1)`,
  drops the carrier statements, and rewrites block 1's log Call args
  to recover the inner u32 operand.
- The wrapper destination local's `LocalDecl.ty` is still nulled to
  unit.
- No new assertion failures.

### B2: `eliminate_residue_removes_wrapper_seed_and_chained_logarg_artifacts` survives Path 1 removal

**Anchor:** `crates/regseq-driver/src/bridge/diag_log/dead_item_elim.rs:1130-1237`.

The test currently passes via Path 1. After Changes A + C it must pass
via Path 2. Verify the existing assertions at `:1187-1235` (wrapper
Instance removed, From-impl Item removed, FnSig removed, LogArg Ty
removed, LogArg ADT Item removed, residue accumulator drained) all hold.

### B3: `eliminate_residue_marks_every_logarg_from_impl_as_dead` survives Path 1 removal

**Anchor:** `crates/regseq-driver/src/bridge/diag_log/dead_item_elim.rs:1266-1397`.

The test currently passes via Path 1. After Changes A + D it must pass
via Path 2. Verify the existing assertions at `:1355-1393` (both
From-impl Items removed, LogArg ADT Item removed, no surviving LogArg
Ty entry) all hold. Update the rendered-string assertions at `:1357-1364`
to match the canonical impl-method form.

### B4: real-bridge integration test stays green (unchanged)

**Anchor:** `crates/regseq-driver/tests/bridge_diag_log_dead_item_elim.rs:392-475`.

This test compiles real source (a non-LogArg `impl From<T> for S` fixture)
through the bridge end-to-end. Its assertions at `:431-475` already
target the canonical impl-method shape (terminal `from` segment with a
`DefPathData::Impl` ancestor, function signature output resolves to the
struct). No changes to this test; it is the normative regression guard
that survives Path 1 removal.

### B5: workspace test suite passes

`cargo test --workspace` green; specifically no regression in the
diag_log inline unit tests, the bridge_diag_log_dead_item_elim
integration test, the regseq-driver-56 bridge_trait_method_resolution
test, the regseq-driver-55 validate_item_instance_coverage tests, or
the regseq-driver-56 e2e_trait_method_resolution snapshot.

## Review

### Files to send to Codex (`scripts/python/codex_runner.py`)

- This file (`.harness/tasks/regseq-driver/57-canonicalize-logarg-from-recognizer.md`).

(No spec amendment is needed for this task; the canonical shape is
already normative per the regseq-driver-56 amendment at
`spec.md:2354-2394`.)

### Reviewer focus

- **Defensive-fallback audit.** Does Change A actually delete Path 1
  rather than leave it dormant? Cite `diag_log/mod.rs` line range before
  and after.
- **Test-fixture canonicality.** Do Changes B / C / D produce Items
  whose def_path segments contain a `DefPathData::Impl` ancestor + a
  terminal `DefPathData::ValueNs("from")`, matching Path 2's structural
  check at `diag_log/mod.rs:67-82`? Cite the produced segment shape.
- **FnSig.output coverage.** Do all three test fixtures construct
  Function Instances whose `FnSig.output` resolves to a LogArg-named
  ADT, satisfying Path 2's signature check at `diag_log/mod.rs:83-90`?
  Cite the FnSig construction lines.
- **Citation drift.** Every file:line in this spec must anchor its
  claim against HEAD.
- **Scope creep.** Does the task touch only the recognizer + three
  fixture builders + doc comments? Per Non-Goals, the real-bridge
  integration test, the eager-enumeration question, and the
  regseq-driver-56 spec amendment are all out of scope. Flag any edit
  outside those bounds.
- **Hidden contract change.** Does removing Path 1 change the
  recognizer's CONTRACT (the set of real-bridge inputs it returns
  `true` for), or only the input shapes it accepts? Cite the
  regseq-driver-56 commit's `bridge_diag_log_dead_item_elim.rs:431-475`
  assertions as the normative real-bridge coverage; the recognizer's
  real-input contract must be unchanged.

### Adversarial prompt template

```
<task>
Review .harness/tasks/regseq-driver/57-canonicalize-logarg-from-recognizer.md.
Identify:
- Citation drift (any file:line that does not anchor the claim).
- Hidden contract changes (does Path 1 removal narrow the recognizer's
  set of real-bridge inputs, or only its accepted input shapes?).
- Test-fixture canonicality (do Changes B/C/D produce Items that
  satisfy Path 2's structural checks?).
- Scope creep beyond the recognizer + three fixture builders + doc
  comments per Non-Goals.
</task>

<grounding_rules>
- Cite file:line for every claim.
- Mark hypotheses as hypotheses.
- Don't speculate about code you didn't read.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED WITH MINOR FIXES | NOT READY
PER-CHANGE VERIFICATION: A, B, C, D, E bullets with file:line
CITATION DRIFT: list or "none"
CONTRACT-CHANGE AUDIT: pass/fail with citation
SCOPE-CREEP CHECK: pass/fail with citation
TEST PLAN VERIFICATION: B1, B2, B3, B4, B5 bullets
NEW ISSUES: numbered list or "none"
REQUIRED FIXES: numbered list (only if VERDICT != APPROVED)
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## Quality Gate

- `cargo fmt --all` clean.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo test --workspace` green (or green with `--test-threads=1` if
  parallel rustc-driver-test stderr interleaves, per the
  regseq-driver-56 commit's observed quirk).
- No leftover trait-method-shape references remain anywhere under
  `crates/regseq-driver/src/bridge/diag_log/`. Confirm via each of
  the following greps returning zero hits:
  - `rg "::From::from" crates/regseq-driver/src/bridge/diag_log/`
    (catches `ends_with("::From::from")`, `core::convert::From::from`,
    `foo::From::from`, `bar::From::from`, and any other literal that
    embeds the trait-method substring).
  - `rg "from_trait_method" crates/regseq-driver/src/bridge/diag_log/`
    (catches the old helper names `make_from_trait_method_item` and
    `push_from_trait_method_item`).
  - `rg -i "trait-method|pre-regseq-driver-56|brittle pre-fix|pre-fix heuristic" crates/regseq-driver/src/bridge/diag_log/`
    (catches doc-comment / inline-comment framings the dual-shape
    cleanup retires, including `trait-method shape`, `trait-method
    callees`, and `trait-method DefId`).
- Codex Task Review verdict: APPROVED or APPROVED WITH MINOR FIXES.
  Zero unaddressed BLOCKERs across review passes.

## Acceptance Criteria

1. `is_from_logarg_instance` (`crates/regseq-driver/src/bridge/diag_log/mod.rs:39-91`)
   matches Path 2 only. The Path 1 block at `:55-61` is deleted; the
   function body consists of bounds checks (`:42-53`) followed by the
   Path 2 structural check.
2. The doc comment at `diag_log/mod.rs:18-37` describes a single
   canonical recognizer (terminal `ValueNs("from")` + `Impl` ancestor +
   `FnSig.output` is a `LogArg`-named ADT), citing the regseq-driver-56
   amendment at `spec.md:2354-2394` as the normative source.
3. `crates/regseq-driver/src/bridge/diag_log/peel.rs:1400-1448`
   constructs an Item whose `def_path.segments` ends with
   `DefPathData::ValueNs("from")` and contains at least one
   `DefPathData::Impl` segment; the rendered string reflects the
   canonical impl-method form.
4. `crates/regseq-driver/src/bridge/diag_log/dead_item_elim.rs:1072-1120`
   has the same structural shape as Change B.
5. `crates/regseq-driver/src/bridge/diag_log/dead_item_elim.rs:1266-1397`
   no longer uses post-hoc `def_path.rendered` overrides for the
   From-impl Items; the two From-impl Items are constructed via a new
   `push_from_impl_method_item` helper with canonical impl-method
   segments + rendered string.
6. All three previously-passing inline unit tests
   (`peels_logarg_from_wrapper_via_structural_recognition`,
   `eliminate_residue_removes_wrapper_seed_and_chained_logarg_artifacts`,
   `eliminate_residue_marks_every_logarg_from_impl_as_dead`) continue
   to pass against Path 2 only.
7. The real-bridge integration test
   `non_logarg_from_impl_in_unrelated_fixture_is_preserved`
   (`crates/regseq-driver/tests/bridge_diag_log_dead_item_elim.rs:392-475`)
   continues to pass unchanged.
8. The full workspace test suite passes; no regression in the
   regseq-driver-55 / regseq-driver-56 coverage cited in the Test Plan.
