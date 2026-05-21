# regseq-driver-65: implementation evidence

Implementation deliverable for `regseq-driver-65`, the F2 first
snippet batch + mini-F3 dispatch slice of the bridge-coverage-runner
per
`.harness/tasks/regseq-driver/65-bridge-coverage-runner-f2-first-batch.md`
and the bridge-coverage-runner strategy at
`docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`
section 11 (phasing -- F2 deliverable interleaving 3 F3 dispatch
arms).

Parent context: this task consumes the F1 scaffold shipped by
regseq-driver-64 (closed at `9f93c2b` with the rpath-fix follow-up
at `e54ddf8`) read-only on architecture and read-write only on
`crates/bridge-coverage-runner/src/inspect.rs` (three new dispatch
arms), `crates/bridge-coverage-runner/matrix_metadata/rows.toml`
(append-only row entries), and `crates/bridge-coverage-runner/
snippets/` (new per-row snippet files). The F0 surface at
`crates/test-ir-fixtures/src/lib.rs` is read-only consumed.

## Scope

- Three new `InspectionPredicate` dispatch arms in
  `crates/bridge-coverage-runner/src/inspect.rs`
  (`SyntheticItemKind`, `ConstVariant`, `CallDivergent`) plus
  three helper variant-tag functions and four module-level unit
  tests.
- 22 new `[[row]]` entries appended to
  `crates/bridge-coverage-runner/matrix_metadata/rows.toml`
  (10 F1 + 22 F2 committed = 32 total). One row (IX-014) is
  escalated-before-attempt per the task spec; one row (IX-017)
  consumed its 3-retry budget without a passing candidate and is
  escalated-after-attempt. Both escalated rows are NOT committed
  to the sidecar.
- 22 new `#![no_std]` snippets under
  `crates/bridge-coverage-runner/snippets/` (one per committed
  F2 row).
- This expectations file.

No edits to F1 runner architecture (`main.rs`, `matrix.rs`,
`snippet.rs`, `oracle.rs`, `cluster.rs`, `report.rs`, `build.rs`,
`Cargo.toml`). No edits to F0. No edits to spec / strategy /
matrix / pretty-printer / generic-c-backend docs.

## Before-fix evidence

- Pre-fix HEAD `44cbc3e` (the F2 task-spec registration commit)
  carried only the F1 runner state: 10 rows in the sidecar, 10
  snippets, 5 implemented `InspectionPredicate` dispatch arms
  (`TyVariant` / `ItemKind` / `FunctionInstanceFlavor` / `All` /
  `Any`), and 17 `Unimplemented` fallthrough arms per
  `crates/bridge-coverage-runner/src/inspect.rs:309-313` (the
  pre-F2 fallthrough block). The matrix has ~133 uncovered rows
  total per
  `docs/discussions/regseq-model/bridge-coverage-matrix.md`
  section 8.
- The 17 unimplemented arms blocked verification of high-yield
  Section-5 interaction rows: a candidate snippet for IX-007
  (dyn Trait x Drop -> VTableDropShim) could compile and produce
  a model containing the synthesised drop shim, but the runner
  could not assert it. The F1 anti-silent-pass rule
  (`Inspection::Unimplemented -> FAIL`) would surface every such
  row as a failure rather than a quiet pass.

## After-fix evidence

### Change A: three new `InspectionPredicate` dispatch arms

- `crates/bridge-coverage-runner/src/inspect.rs:215-245` add the
  helper functions `synthetic_kind_variant_tag` (10 variants:
  None / ClosureCaptures / ClosureCall / ClosureDropGlue /
  DropGlue / VTableType / VTableInstance / VTableMethodShim /
  VTableDropShim / ConstMemoryStatic) and `const_variant_tag`
  (11 variants: Bool / Char / Int / Uint / Float / ZST / Bytes /
  Aggregate / EnumValue / NullPtr / Reloc). The functions live
  adjacent to the existing F1 helpers (`ty_variant_tag`,
  `item_kind_tag`, `flavor_tag`) per the spec's "adjacent
  naming + style" requirement.
- `crates/bridge-coverage-runner/src/inspect.rs:293-329` add the
  `InspectionPredicate::SyntheticItemKind`,
  `InspectionPredicate::ConstVariant`, and
  `InspectionPredicate::CallDivergent` dispatch arms. Each walks
  the relevant model slice (`model.item_synthetic_kinds`,
  `model.consts.iter()`, `model.instances.iter()` + Function
  body basic-blocks) and returns `Ok(())` on first match,
  `Err(InspectError::NotFound { variant, arg })` otherwise. The
  `CallDivergent` arm departs from the spec's literal text
  `bb.terminator.as_ref()` because `BasicBlock.terminator` is a
  non-optional `Terminator` per
  `crates/regseq-model/src/basic_block.rs:45`; the dispatch
  matches `bb.terminator.kind` directly. Behavior is unchanged.
- Four module-level unit tests at
  `crates/bridge-coverage-runner/src/inspect.rs` exercise each
  arm:
  - `synthetic_item_kind_arm_matches_present_variant_and_reports_not_found_otherwise`
    constructs a model carrying one `SyntheticItemKind::VTableDropShim`
    entry and asserts `Ok(())` for the matching tag plus
    `NotFound { variant: "SyntheticItemKind", .. }` for a
    distinct tag.
  - `const_variant_arm_matches_present_variant_and_reports_not_found_otherwise`
    interns `Const::Float` and `Const::Int` values and asserts
    both predicates pass plus the absent `Reloc` tag surfaces
    `NotFound`.
  - `call_divergent_arm_matches_divergent_call_and_reports_not_found_otherwise`
    builds two minimal Function instances (one with a
    `target: None` call, one with `target: Some(_)`) and
    asserts the `true` / `false` arg variants match the right
    body shapes plus produce `NotFound` on the inverse.
  - `unimplemented_arms_still_report_unimplemented` is a
    regression guard ensuring the 14 remaining unimplemented
    leaf arms still return `InspectError::Unimplemented` (the
    spec's anti-silent-pass invariant).

### Change B: 22 committed sidecar rows + 22 committed snippets

The runner accepts the expanded sidecar:

- `Total rows = 32` (10 F1 + 22 F2 committed).
- `Pass = 32`, `Fail = 0`, exit code 0.
- `Outcomes by class: Ok=16, Rejected=16`.

Per-row evidence (final inspection predicate / final snippet
shape / actual outcome). Citations point at the final committed
versions at the snippets dir and rows.toml.

#### Section 5 (positive rows)

| Row ID | Final predicate | Final snippet shape (path) | Actual outcome | Status |
| ------ | --------------- | -------------------------- | -------------- | ------ |
| IX-001 | `All([ItemKind=Function, FunctionInstanceFlavor=Item])` | `snippets/IX-001.rs` -- `pub fn id<T>(x: T) -> T { x }` + `pub fn entry() -> u32 { id::<u32>(0) }` | Ok, inspection passed | PASS (round 1) |
| IX-007 | `SyntheticItemKind=VTableDropShim` | `snippets/IX-007.rs` -- trait `T` with method `m`, `pub struct S`, `impl T for S`, `impl Drop for S`, `fn f(s: &S) -> u32 { let d: &dyn T = s; d.m() }` | Ok, inspection passed | PASS (round 2) -- round 1 used `*mut dyn T` + `drop_in_place`; the bridge rejected `dyn T` (UnsupportedTy) because the round-1 trait was empty (no methods). Refined to TY-014-A's working dyn-shape with Drop on S. |
| IX-008 | `SyntheticItemKind=VTableMethodShim` | `snippets/IX-008.rs` -- trait `T` with `m`, impl, `&dyn T` cast + method call | Ok, inspection passed | PASS (round 1) |
| IX-012 | `All([FunctionInstanceFlavor=DropGlue, ItemKind=Struct])` | `snippets/IX-012.rs` -- `Inner` with `Drop`, `Outer { i: Inner }` without explicit Drop, scope construct | Ok, inspection passed | PASS (round 2) -- round 1 had `impl Drop for Outer` AND `impl Drop for Inner`; the synthesised joint drop glue body tripped Phase 5 BRpo validation. Refined to drop Outer's explicit Drop impl; the tree-drop glue Outer synthesises still includes a DropGlue function instance + the Inner+Outer Struct items. |
| IX-013 | `SyntheticItemKind=ClosureDropGlue` | `snippets/IX-013.rs` -- closure move-captures `R: Drop`, `let _ = c;` immediately drops the closure without calling it | Ok, inspection passed | PASS (round 2) -- round 1 invoked the closure via `c()` which hits the IX-014/IX-004 FnOnce family bug. The refinement avoids calling the closure entirely; the closure-environment drop still synthesises `ClosureDropGlue` when the closure is dropped at end-of-scope. |
| ~~IX-014~~ | DEFERRED -- not committed | -- | escalated-before-attempt | ESCALATED (pre-F2, per task spec) -- the natural direct-consumption snippet panics the bridge AND maps to `FunctionInstanceFlavor::Item` rather than `ClosureOnceShim` per `crates/regseq-driver/src/bridge/body/terminator.rs:290-369`; the generic-FnOnce alternative hits the IX-004/IX-010 OPEN BUG family. Follow-up direction recorded in §"Escalated rows" below. |
| IX-015 | `All([ItemKind=Function, ItemKind=Struct])` | `snippets/IX-015.rs` -- generic struct `W<T>` + generic fn taking `W<T>` and returning the field + `entry` calling with `u32` | Ok, inspection passed | PASS (round 1) |
| ~~IX-017~~ | DEFERRED -- not committed | -- | escalated-after-attempt | ESCALATED (during F2, after 1 implemented + 1 hypothesis-mismatch refinement attempt) -- see §"Escalated rows" below. The hypothesis `FunctionInstanceFlavor=Intrinsic` does not match the bridge's post-peel model shape for regseq diag-log functions; the peel pass leaves the log_info function as `FunctionInstanceFlavor::Item` with `body: None`, and the only distinguishing signal lives in `model.domain_attrs.item_attrs` as `AttrData::Intrinsic(IntrinsicKind::Log(_))`. None of the eight implemented predicates can distinguish that artifact. |
| IX-019 | `FunctionInstanceFlavor=DropGlue` | `snippets/IX-019.rs` -- two adjacent `let _x = R;` `let _y = R;` inside one fn body, with `R: Drop` | Ok, inspection passed | PASS (round 1) |
| IX-021 | `TyVariant=Dynamic` | `snippets/IX-021.rs` -- `fn f(_d: &(dyn T + Send))` with a locally-defined `pub trait T {}` (avoiding `core::fmt::Debug` which would need `alloc`) | Ok, inspection passed | PASS (round 1) |
| IX-022 | `ConstVariant=Reloc` | `snippets/IX-022.rs` -- `pub static X: u32 = 42; pub const P: &'static u32 = &X; pub fn entry() -> u32 { *P }` -- `&X` in const context evaluates to a `GlobalAlloc::Static` relocation that the bridge maps to `Const::Reloc(InstanceRef)` per `crates/regseq-driver/src/bridge/const_.rs:414-426` | Ok, inspection passed | PASS (round 1) |
| IX-023 | `CallDivergent=true` | `snippets/IX-023.rs` -- `pub fn boom() -> ! { loop {} }` + `pub fn entry() { boom() }` | Ok, inspection passed | PASS (round 1) |

#### Section 6 (rejected rows)

| Row ID | Expected rejection | Final snippet shape (path) | Actual outcome | Status |
| ------ | ------------------ | -------------------------- | -------------- | ------ |
| RJ-003 | `DslRejection` (kind=`Atomic`) | `snippets/RJ-003.rs` -- `pub static A: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);` | Rejected, variant=DslRejection | PASS (round 1) |
| RJ-005 | `DslRejection` (kind=`Rvalue`) | `snippets/RJ-005.rs` -- `#[thread_local] pub static T: u32 = 0; pub fn f() -> u32 { unsafe { T } }` with `#![feature(thread_local)]` | Rejected, variant=DslRejection | PASS (round 1) |
| RJ-019 | `DynAutoOnly` | `snippets/RJ-019.rs` -- `pub fn f(_d: &dyn Send) {}` | Rejected, variant=DynAutoOnly | PASS (round 1) |
| RJ-022 | `SubstantiveHrtb` | `snippets/RJ-022.rs` -- `pub fn f(_: for<'a> fn(&'a u32, &'a u32)) {}` (both lifetime uses share one binder = substantive per `crates/regseq-driver/src/bridge/ty.rs:1471-1477` and confirmed by `crates/regseq-driver/tests/bridge_ty.rs:554-577`'s `hrtb` rejection) | Rejected, variant=SubstantiveHrtb | PASS (round 1) |
| RJ-027 | `UnsupportedNeverInput` | `snippets/RJ-027.rs` -- `#![feature(never_type)] pub fn f(_x: !) {}` | Rejected, variant=UnsupportedNeverInput | PASS (round 1) |
| RJ-028-B | `UnsupportedCAbiType` (position=`Output`) | `snippets/RJ-028-B.rs` -- `pub extern "C" fn f() -> (u32, u32) { (1, 2) }` | Rejected, variant=UnsupportedCAbiType | PASS (round 1) |
| RJ-028-C | `UnsupportedCAbiType` (position=`Field`) | `snippets/RJ-028-C.rs` -- `#[repr(C)] pub struct WithRustFn { f: fn() -> u32 } pub extern "C" fn boundary(_s: WithRustFn) {}` -- known-good shape per `crates/regseq-driver/tests/bridge_fnsig.rs:398-413` | Rejected, variant=UnsupportedCAbiType | PASS (round 1) |
| RJ-031-A | `DiagLogFormat` (reason=`DebugFormatterRejected`) | `snippets/RJ-031-A.rs` -- inlined log shim + `regseq::log::info!("x={:?}", 0_u32)` | Rejected, variant=DiagLogFormat | PASS (round 1) |
| RJ-031-B | `DiagLogFormat` (reason=`PositionalArgRejected`) | `snippets/RJ-031-B.rs` -- inlined log shim + `regseq::log::info!("x={0}", 0_u32)` | Rejected, variant=DiagLogFormat | PASS (round 1) |
| RJ-031-C | `DiagLogFormat` (reason=`NamedArgRejected`) | `snippets/RJ-031-C.rs` -- inlined log shim + `regseq::log::info!("x={name}", 0_u32)` | Rejected, variant=DiagLogFormat | PASS (round 1) |
| RJ-031-E | `DiagLogFormat` (reason=`UnclosedBrace`) | `snippets/RJ-031-E.rs` -- inlined log shim + `regseq::log::info!("x={", 0_u32)` | Rejected, variant=DiagLogFormat | PASS (round 1) |
| RJ-031-F | `DiagLogFormat` (reason=`UnmatchedClosingBrace`) | `snippets/RJ-031-F.rs` -- inlined log shim + `regseq::log::info!("x=}", 0_u32)` | Rejected, variant=DiagLogFormat | PASS (round 1) |

Note: the F1 runner's `expected_rejection_matches` at
`crates/bridge-coverage-runner/src/main.rs:264-299` only enforces
the `variant` field for `DiagLogFormat` (no `reason` sub-axis
check today); the `DiagLogFormatReason` precision is recorded
above and in the per-row construct strings, and a follow-up F3 /
F4 task may extend the matcher with a `reason = ...` qualifier.

#### Drift status

- `Total rows = 32`. `Pass = 32`. `Fail = 0`.
- `Sidecar rows missing from matrix`: none.
- `Matrix rows missing from sidecar`: 327 (the remaining
  uncovered surface; subset-OK per strategy section 4.1).

### Change C: per-snippet authoring + verification loop evidence

Each F2 row's snippet was authored, added to the sidecar, and
verified against the runner's `report.md` per Change C of the
task spec. Round-by-round empirical outcomes:

- **Round 1**: 23 candidate rows attempted (IX-014 excluded
  per the task spec's escalated-before-attempt note). 19 PASS
  + 4 FAIL.
  - IX-007 FAIL: bridge rejected `dyn T` as `UnsupportedTy`
    (empty-trait dyn through `*mut dyn T`).
  - IX-012 FAIL: Phase 5 BRpo body-validation failure on the
    joint drop glue body synthesised by the bridge for
    Outer-with-Drop containing Inner-with-Drop.
  - IX-013 FAIL: bridge panic (matching the IX-004/IX-010
    open-bug family) when the closure is invoked via
    `c()` (FnOnce path).
  - IX-017 FAIL: Diag-log validation D1 (placeholder count
    mismatch -- the macro discarded all `$arg:expr` args while
    the `{}` placeholder remained in the format string).
- **Round 2**: refined the 4 FAIL candidates. IX-007 / IX-012 /
  IX-013 PASS after refinement (see per-row "Status" notes
  above). IX-017 PASS at the bridge level but the inspection
  predicate `FunctionInstanceFlavor=Intrinsic` does not match
  the post-peel artifact -- bridge sets `body: None` on the
  log_info instance with `FunctionInstanceFlavor::Item`, not
  `Intrinsic`. The "Intrinsic" flavor is reserved for rustc
  compiler intrinsics per
  `crates/regseq-driver/src/bridge/mod.rs:2511`; regseq's
  `#[rseq::intrinsic(...)]` attribute populates
  `model.domain_attrs.item_attrs` with
  `AttrData::Intrinsic(IntrinsicKind::Log(_))`, which none of
  the eight implemented predicates can inspect.
- **Round 3**: IX-017 escalated; the row entry was not
  committed to the sidecar and the snippet file is not present.
  The remaining 22 F2 rows pass.

The 3-retry budget per row was never exceeded. IX-017's two
attempts (round-1 placeholder-count mismatch + round-2
hypothesis-mismatch refinement that still cannot be verified by
any implemented predicate) are documented as ESCALATED rather
than retried again at the snippet level, since the gap is
predicate vocabulary, not snippet shape.

### Change D: harness expectations file

This file. Covers per-row outcomes, the mini-F3 deviation, the
two escalations (IX-014 + IX-017), and the F3 readiness signal.

## Escalated rows

### IX-014 -- FnOnce x Move consumption (escalated-before-attempt)

Per task spec §"Description". Two failed candidate shapes were
empirically probed in Codex round-2 before the F2 task spec was
finalised:

- **Candidate A**: direct closure consumption -- panics the
  bridge AND maps to `FunctionInstanceFlavor::Item` (NOT
  `ClosureOnceShim`) per
  `crates/regseq-driver/src/bridge/body/terminator.rs:290-369`.
- **Candidate B**: generic `F: FnOnce` bound -- hits the
  IX-004 / IX-010 OPEN BUG family per the matrix.

Follow-up direction: either (a) a future task adds a
sub-variant-matching `SyntheticItemKind` predicate (e.g.,
`SyntheticItemKind=ClosureCall, sub=CallOnce`) and uses the
direct-consumption path's actual outcome shape, or (b) a separate
bug-discovery task files the bridge panic + the OPEN BUG family
investigation upstream.

The snippet file is not present in `snippets/`; the row entry is
not committed to the sidecar.

### IX-017 -- Diag-log intrinsic x generic caller (escalated-after-attempt)

Per the round-2 evidence above. Candidate shapes attempted:

- **Candidate A (round 1)**: full diag-log shim + macro call
  `regseq::log::info!("x={}", 0_u32)`. Outcome: bridge rejected
  with `BridgeError::DiagLogValidation { reason: D1, .. }` --
  the macro expands the args to `&[]` (the F0 shim macro
  discards `$arg:expr` tokens), so the format string has 1
  placeholder while the peeled call carries 0 args. The peel
  pass's D1 validation enforces parity.
- **Candidate B (round 2)**: same shim, call refined to
  `regseq::log::info!("event")` (no placeholders). Outcome:
  bridge accepted (Ok), but the inspection predicate
  `FunctionInstanceFlavor=Intrinsic` did NOT match. The
  post-peel log_info function instance has
  `FunctionInstanceFlavor::Item` (not `Intrinsic`) and
  `body: None`. The peel pass leaves the only distinguishing
  signal in `model.domain_attrs.item_attrs` as
  `AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Info))`,
  which none of the implemented predicates can inspect.

Follow-up direction: F3 / F4 should add a `DomainAttr` /
`IntrinsicAttr` predicate (one of the 14 currently-unimplemented
leaf variants) that walks `model.domain_attrs.item_attrs` and
matches `AttrData::Intrinsic(IntrinsicKind::Log(_))` -- OR add a
`BodyAbsent` predicate (`InspectionPredicate::BodyAbsent` enum
variant ships in F1 but dispatch is unimplemented) that asserts
at least one Function instance carries `body: None`. Either
predicate would let a future F4 batch re-attempt IX-017 against
the existing snippet shape.

The snippet file is not present in `snippets/`; the row entry is
not committed to the sidecar.

## Strategy-deviation record: mini-F3 inside F2

The strategy at
`docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`
section 11 schedules F2 ("First snippet batch") strictly before
F3 ("Inspection-predicate dispatch hardening"). F2 deviates by
INTERLEAVING the smallest possible F3 slice (3 dispatch arms)
required by the chosen Section-5 row set:

- `InspectionPredicate::SyntheticItemKind` -- needed by IX-007
  (`VTableDropShim`), IX-008 (`VTableMethodShim`), IX-013
  (`ClosureDropGlue`).
- `InspectionPredicate::ConstVariant` -- needed by IX-022
  (`Reloc`).
- `InspectionPredicate::CallDivergent` -- needed by IX-023
  (`true`).

The deviation is bounded: F2 does NOT touch the remaining 14
unimplemented arms. F3's scope is fully bounded to the
predicate vocabulary listed in the F3 readiness signal below.

## F3 readiness signal

The runner's dispatch coverage is now operationally sufficient
for F3 to focus on the remaining 14 unimplemented predicates
WITHOUT further mini-F3 interleaves in subsequent F* phases.
The 14 remaining `Unimplemented` leaf variants are:

1. `StatementKind`
2. `TerminatorKind`
3. `RvalueVariant`
4. `PlaceElem`
5. `AggregateKind`
6. `CastKind`
7. `BinOp`
8. `UnOp`
9. `AssertKind`
10. `NonDivergingIntrinsic`
11. `CallCallee`
12. `BodyAbsent`
13. `Linkage`
14. `ScalarKind`

`crates/bridge-coverage-runner/src/inspect.rs:34-82` defines 22
`InspectionPredicate` variants total: 20 leaf variants plus the
two composites (`All` / `Any`). F1 implemented 5 leaves +
both composites; F2 adds 3 leaves; 14 leaves remain
`Unimplemented`. The locked fact for F3 is: those 14 leaf
variants currently return `InspectError::Unimplemented`; F3
implements them. No further mini-F3 interleaves are planned for
F4.

The two escalated rows (IX-014, IX-017) each block on a
specific F3 / F4 work item:

- IX-014 -- needs the IX-004/IX-010 OPEN BUG family resolved
  upstream OR a separate sub-variant `SyntheticItemKind`
  predicate refinement.
- IX-017 -- needs a `DomainAttr` / `IntrinsicAttr` predicate
  OR a `BodyAbsent` dispatch arm (the latter ships as enum
  variant in F1 but is currently `Unimplemented`).

F3 may begin its dispatch-hardening batch against this runner
state.

## Out of scope (per task Non-Goals)

- Remaining 14 `InspectionPredicate` dispatch arms (F3).
- F1 runner architecture modifications (the seven src modules,
  `build.rs`, `Cargo.toml`); F2 only modifies `inspect.rs`.
- F0 helper modifications: `git diff main --
  crates/test-ir-fixtures/` is empty.
- spec.md / pretty-printer.md / generic-c-backend.md /
  bridge-coverage-matrix.md / bridge-coverage-runner-strategy.md
  changes -- none.
- More than 23 attempted rows. F2 attempted 23 (IX-014 +
  the 22 committed + IX-017 escalated). F4 picks up the
  remaining ~109 uncovered matrix rows.
- New dependencies beyond F1's set (`test-ir-fixtures`,
  `regseq-model`, `regseq-driver`, `toml`, `serde + derive`,
  `sha2`).
- RJ-031-D (`NonSubsetModifier`) -- deferred to F4 per the
  task spec's Non-Goals.
- Validator-failing rows (Section 7 of the matrix).
- Rows blocked-open-bridge-bug (IX-004 / IX-010).
- Modifications to `tests/fixtures/playground/`.
- Single-shot LLM authoring of all 23 snippets; per-row 3-retry
  loop ran sequentially as specified.
