# regseq-driver-67: Full snippet corpus batch (~100 rows) (F4 of bridge-coverage-runner)

## Description

F4 of the bridge-coverage-runner strategy per
`docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`
§11. Land the remaining uncovered matrix rows as committed
sidecar entries + snippets, exercising the now-complete
`InspectionPredicate` dispatch surface shipped by F3 (22/22 arms
implemented).

Building on F3 (regseq-driver-66, closed at `e2e77c0`): the
runner ships with the full predicate vocabulary (20 leaves + 2
composites), sub-axis matcher precision for rejections
(`kind` / `position` / `reason` / `diagnostic`), and 32
committed sidecar rows. F4 extends the sidecar by up to ~100
additional rows -- one per remaining empirically-authorable
uncovered matrix row.

F4 is the **corpus completion** phase. Per strategy §11 this
phase is parallelizable across agents and estimated 5-8
agent-hours; in practice the per-row 3-retry authoring loop runs
sequentially against a single sidecar, so realistic wall time is
~8-12h with possible escalations. F4 also includes the IX-017
re-attempt unblocked by F3's `BodyAbsent` dispatch arm (per the
F3 expectations file's B6 probe outcome = YES).

After F4 lands, **batch triage** begins (separate follow-up
task): walk every F2/F3/F4 expectations file + the final F4
report.md to cluster findings and propose `regseq-driver-NN`
investigation tasks for human approval (per strategy §10 Mode B).

### Authoring protocol (inherited from F2/F3)

Per strategy §5: for each candidate row, the implementer authors
a `#![no_std]` snippet (with whatever feature gates the shape
requires); the runner verifies via the typed-outcome oracle; if
compile + inspection-pass (positive) OR compile + sub-axis-precise
rejection (negative), the candidate is accepted. Up to 3 retries
before the row escalates. **Escalated rows stay OUT of the
committed sidecar entirely** (same protocol as F2).

After every accepted row (or coherent cluster of related rows),
the implementer commits the snippet + sidecar entry together.
The runner's drift check at every commit boundary enforces the
sidecar-is-subset-of-matrix invariant.

### Section coverage targets

Per the matrix doc (`docs/discussions/regseq-model/bridge-coverage-matrix.md`):

| Matrix section | Authorable uncovered rows (F4 target) | Notes |
| -------------- | ------------------------------------- | ----- |
| 1.1 `Ty` enum + sub-axes | ~22 | TY-001 / TY-002 / TY-007 / TY-008 / TY-011-A / TY-016; TY-003 sub-axes (5); TY-004 sub-axes (3); TY-005 sub-axes (4 with feature gates for F16/F128); TY-014-B/C/D/E (auto-trait combos -- loose `TyVariant=Dynamic` predicate) |
| 1.2 `PlaceElem` | 3 | PL-004 / PL-005 / PL-006 |
| 1.3 `Operand`/`Const` sub-axes | ~6 | OP-003-A/B/E/H/I/J (OP-003-G-1/G-3 deferred -- no F3 Byte sub-variant predicate; OP-003-K deferred -- already covered by IX-022) |
| 1.4 `Rvalue` sub-axes | ~45 | RV-002; RV-005 sub-axes (13); RV-006 sub-axes (24, B-Z minus I); RV-007 sub-axes (3); RV-009 sub-axes (4) |
| 1.5 `Statement` | 2 | ST-003-A / ST-003-B |
| 1.6 `Terminator` sub-axes | ~5 | TM-004 / TM-006-A / TM-006-C / TM-006-D / TM-006-E (F/G/H deferred; TM-005-D covered by IX-023) |
| 2.3 `FFlavor` | 4 | FF-004 / FF-005 / FF-012 / FF-013 (FF-007 blocked) |
| 3 source-level features | 3 | FT-002 / FT-005 / FT-008 (FT-003/FT-009 covered by IX-012/IX-001 aliasing; FT-004 ManuallyDrop deferred -- no negative predicate; FT-012/013/014 blocked) |
| 4 compilation/toolchain | 1 | CT-011 (CT-010 deferred -- no current `Linkage=Weak` producer in bridge; CT-004/007/008 documented blocks) |
| 5 interactions | 1 | IX-017 re-attempt via BodyAbsent |
| 6 rejections | ~10 | RJ-006 / RJ-015 / RJ-024 / RJ-031-D / RJ-032-A / RJ-032-B / RJ-032-C / RJ-032-D / RJ-032-E / RJ-032-F |

**Target**: ~102 candidate rows. Realistic commit count: 60-80
after the 3-retry escalation filter (some rows are noted with
"likely escalation" risk in the per-section tables below).

### Out-of-scope rows (documented blocks)

The following uncovered matrix rows are NOT F4 candidates:

- `blocked-rustc-infeasible`: RJ-014 (Error), RJ-017 (Pat),
  RJ-020 (DynStar), RJ-021 (CustomAutoTrait). Per matrix
  Section 6.2 + 6.3.
- `blocked-bridge-rejects-by-design` with no positive-coverage
  shape: FF-008 (Coroutine shim, covered indirectly via RJ-016
  in F1), FF-009 (TLS shim), FF-010 (FutureDropPoll), FF-014 /
  FF-015 (AsyncDrop).
- `blocked-open-bridge-bug`: IX-004 / IX-010 / FT-011 / IX-014
  (the OPEN BUG family). Defer to a separate bug-investigation
  task after batch triage.
- `blocked-spec-gap`: any matrix row marked thus (none currently
  in 1a scope, but recorded as the canonical filter).
- Section 7 (validator invariants, ~60 rows): constructed-model
  tests, separate track per strategy §2 "Out of scope (initial
  landing)".
- Section 2.2 `SyntheticItemKind` sub-axes blocked by closure
  invocation: SK-004 (FnMut), SK-005 (FnOnce). Both rely on
  generic closure-call resolution, blocked by IX-004/IX-010.
- Section 3 closure features blocked: FT-012 / FT-013 / FT-014
  (closure with N captures / FnMut / FnOnce). Task-level
  blocker via the IX-004/IX-010 closure-call family per
  `.harness/expectations/regseq-driver-65.md:223-247` IX-014
  evidence; not matrix-marked blocked-* but empirically blocked.
- Section 4 macro/multi-crate features: CT-004 (Edition 2024,
  cross-edition complexity), CT-007 (alloc dep, requires
  `extern crate alloc;` outside the helper's setup), CT-008
  (multi-crate workspace, out of single-snippet shape).
- Section 6 hard-to-author rejections: RJ-001 (Box, requires
  alloc), RJ-004 (PlaceProjection, no known surface from
  `#![no_std]` source per matrix), RJ-007 (Yield, blocked by
  the RJ-016 Coroutine prerequisite via the bridge's coroutine
  rejection at the Ty-translation layer), RJ-009 / RJ-010 /
  RJ-011 / RJ-012 / RJ-013 (NonRigidTy family beyond what
  IX-001's monomorphization covers), RJ-018 (UnsafeBinder,
  nightly), RJ-023 (Layout, no clean repro), RJ-025
  (UnsupportedTy, no clean repro), RJ-029 (UnsupportedConst,
  complex), RJ-030 (GenericOpaqueHandle, `#[rseq::opaque]`
  setup).
- Section 2.3 `FFlavor` blocked by IX-014 family: FF-007
  (ClosureOnceShim) -- shares the same closure-invocation
  blocker. FF-005 (FnPtrShim) IS addressed by F4 (see Change A
  Section 2.3 table) -- it does not require generic closure-call
  resolution.
- Rows already-covered-by-IX-aliasing: OP-003-K (covered by
  IX-022's `ConstVariant=Reloc`), TM-005-D (covered by
  IX-023's `CallDivergent=true`), FT-003 (covered by IX-012's
  loose nested-Drop proxy), FT-009 (covered by IX-001's
  generic-fn-concrete-args proxy). The matrix retains these as
  distinct row-IDs but their EMPIRICAL coverage is already
  asserted by the F2-committed IX rows; F4 does not re-commit
  them as standalone rows. A future matrix-precision pass may
  annotate the matrix to reflect this aliasing.
- Rows with no F3-implemented predicate: OP-003-G-1
  (`Byte::Uninit`), OP-003-G-3 (`Byte::Reloc`). The F3
  `ConstVariant` dispatch matches `Bytes` but does NOT
  distinguish inner `Byte` sub-variants per
  `crates/regseq-model/src/const_value.rs:69`. F4 defers
  these; a future task may add a `ByteVariant` predicate.
- FT-004 (`ManuallyDrop<T>` NEGATIVE assertion): the runner
  has no native "predicate must NOT match" support. F4 defers;
  a future task may add a `Not<P>` composite predicate.
- CT-010 (`Linkage` enum sub-axes): three variants
  (`Default` / `NoMangle` / `Foreign`) are covered by F1+F2
  fixtures via implicit `Linkage` field values. The `Weak`
  variant is NOT a current bridge producer per
  `crates/regseq-driver/src/bridge/mod.rs:908-955` -- the
  bridge sets only `Default` / `NoMangle` / `Foreign` on
  rendered items. Until the bridge gains a `Weak` producer
  (separate task), `Linkage=Weak` cannot pass; F4 defers
  CT-010 entirely.

Each block category is documented inline; the implementer does
NOT attempt these rows. Future tasks may unblock specific
entries.

## Input Context

### Design source

- `docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`
  §5 (snippet authoring strategy, 3-retry protocol), §10 (Mode
  A invocation), §11 (F4 = ~100-row batch, parallelizable
  across agents, 5-8 agent-hours).

### Matrix source

- `docs/discussions/regseq-model/bridge-coverage-matrix.md`
  (H1 title at `:1` reads "v2" pending editorial sweep; the
  change-log section reflects the v2.1 revision and the body
  carries v2.1 row classifications -- consistent with the
  strategy-doc title/body version mismatch addressed in F2/F3
  specs). Per-section sub-tables for the F4 row lists below.
  The matrix is the authority for which rows are uncovered;
  F4's drift check at every commit boundary enforces sidecar
  IDs exist in matrix IDs.

### F3 runner state

- `crates/bridge-coverage-runner/src/inspect.rs` -- 22/22
  dispatch arms implemented (20 leaves + 2 composites).
  `InspectError` carries only `NotFound` + `AnyFailed`; the
  `Unimplemented` variant was removed by F3 (no producer after
  full dispatch).
- `crates/bridge-coverage-runner/src/main.rs:264+` --
  `expected_rejection_matches` enforces all four sub-axes
  (`kind` / `position` / `reason` / `diagnostic`) with
  `Option::None => no constraint` semantics.
- `crates/bridge-coverage-runner/matrix_metadata/rows.toml` --
  32 sidecar rows (10 F1 + 22 F2; F3 uplifted 5 rows with
  `reason` precision but added no new rows).
- `crates/bridge-coverage-runner/snippets/` -- 32 snippets.
- `crates/bridge-coverage-runner/snippets/README.md` -- naming
  convention + the F3-added CastKind dotted-compound
  `"PointerCoercion::<sub>"` arg convention.

### F2/F3 expectations files (escalation precedents)

- `.harness/expectations/regseq-driver-65.md:223-281` -- IX-014
  + IX-017 escalation records. F4 inherits IX-014 deferral; F4
  re-attempts IX-017 with the new `BodyAbsent` predicate.
- `.harness/expectations/regseq-driver-66.md:287-312` -- F3 B6
  probe outcome documenting that `BodyAbsent` matches the
  IX-017 round-2 snippet's model.

### Prior tasks

- regseq-driver-66 (closed at `e2e77c0`): F3 dispatch hardening.
- regseq-driver-65 (closed at `3a01888`): F2 first batch + mini-F3.
- regseq-driver-64 (closed at `9f93c2b`, rpath fix `e54ddf8`):
  F1 scaffold.

## Implementation Notes

### Change A: row enumeration (per-section tables)

Each section lists `row_id`, `construct`, `predicate or
expected_rejection`, and `notes` (snippet-shape hint or
escalation risk). The implementer authors snippets per the F2/F3
3-retry protocol; rows that escalate are documented per Change C.

For ROW IDs not listed here, refer to the matrix doc directly;
F4 does not commit rows outside this list.

#### Section 1.1 -- `Ty` enum + sub-axes (~22 rows)

| Row ID | Construct | Predicate | Notes |
| ------ | --------- | --------- | ----- |
| TY-001 | `Ty::Bool` | `TyVariant=Bool` | `pub fn f(b: bool) -> bool { !b }` |
| TY-002 | `Ty::Char` | `TyVariant=Char` | `pub fn f(c: char) -> char { c }` |
| TY-007 | `Ty::Never` | `TyVariant=Never` | `pub fn f() -> ! { loop {} }` or `panic!()` |
| TY-008 | `Ty::Array(_, _)` | `TyVariant=Array` | `pub fn f() -> [u32; 4] { [0; 4] }` |
| TY-011-A | `Ty::Ptr(_, Not)` | `TyVariant=Ptr` (loose; F3 does not distinguish Mutability) | `pub fn f(p: *const u32) -> *const u32 { p }` |
| TY-016 | `Ty::Tuple` (non-unit) | `TyVariant=Tuple` | `pub fn f(t: (u32, u32)) -> u32 { t.0 }` |
| TY-014-B | `dyn T + Send` | `TyVariant=Dynamic` (loose; auto-traits not distinguished by F3 predicate) | `pub trait T {} pub fn f(_: &(dyn T + Send)) {}` |
| TY-014-C | `dyn T + Sync` | `TyVariant=Dynamic` (loose) | similar shape w/ Sync |
| TY-014-D | `dyn T + Unpin` | `TyVariant=Dynamic` (loose) | similar w/ Unpin |
| TY-014-E | `dyn T + Send + Sync` | `TyVariant=Dynamic` (loose) | similar w/ both |
| TY-003-A | `IntKind::I8` | `ScalarKind=I8` | `pub fn f(x: i8) -> i8 { x }` |
| TY-003-B | `IntKind::I16` | `ScalarKind=I16` | similar |
| TY-003-D | `IntKind::I64` | `ScalarKind=I64` | similar |
| TY-003-E | `IntKind::I128` | `ScalarKind=I128` | similar |
| TY-003-F | `IntKind::Isize` | `ScalarKind=Isize` | similar |
| TY-004-B | `UintKind::U16` | `ScalarKind=U16` | similar w/ u16 |
| TY-004-E | `UintKind::U128` | `ScalarKind=U128` | similar w/ u128 |
| TY-004-F | `UintKind::Usize` | `ScalarKind=Usize` | similar w/ usize |
| TY-005-A | `FloatKind::F16` | `ScalarKind=F16` | needs `#![feature(f16)]`; escalation risk if pinned nightly rejects |
| TY-005-B | `FloatKind::F32` | `ScalarKind=F32` | `pub fn f(x: f32) -> f32 { x }` |
| TY-005-C | `FloatKind::F64` | `ScalarKind=F64` | `pub fn f(x: f64) -> f64 { x }` |
| TY-005-D | `FloatKind::F128` | `ScalarKind=F128` | needs `#![feature(f128)]`; escalation risk |

Note on TY-014-B/C/D/E loose predicates: all four resolve to
`TyVariant=Dynamic` because F3 did NOT add an auto-trait-set
sub-predicate (it would have been a new enum variant, out of
F3 scope). Coverage is real (each snippet exercises a distinct
auto-trait combination through the bridge) but the runner can't
distinguish them in inspection. Future task may add a more
precise predicate.

#### Section 1.2 -- `PlaceElem` (3 rows)

| Row ID | Construct | Predicate | Notes |
| ------ | --------- | --------- | ----- |
| PL-004 | `PlaceElem::Index` | `PlaceElem=Index` | `pub fn f(a: [u32; 4], i: usize) -> u32 { a[i] }` |
| PL-005 | `PlaceElem::ConstantIndex` | `PlaceElem=ConstantIndex` | slice pattern `let [first, ..] = arr` |
| PL-006 | `PlaceElem::Subslice` | `PlaceElem=Subslice` | slice pattern `let [head, .., tail] = arr` -- may escalate if rustc doesn't lower the way the predicate expects |

#### Section 1.3 -- `Operand`/`Const` sub-axes (6 rows)

| Row ID | Construct | Predicate | Notes |
| ------ | --------- | --------- | ----- |
| OP-003-A | `Const::Bool` | `ConstVariant=Bool` | `pub fn f() -> bool { true }` |
| OP-003-B | `Const::Char` | `ConstVariant=Char` | `pub fn f() -> char { 'a' }` |
| OP-003-E | `Const::Float` | `ConstVariant=Float` | `pub fn f() -> f32 { 1.0 }` |
| OP-003-H | `Const::Aggregate` | `ConstVariant=Aggregate` | `pub const A: [u32; 2] = [1, 2];` |
| OP-003-I | `Const::EnumValue` | `ConstVariant=EnumValue` | `pub enum E { A, B } pub const V: E = E::A;` |
| OP-003-J | `Const::NullPtr` | `ConstVariant=NullPtr` | `pub fn f() -> *const u32 { core::ptr::null() }` |

#### Section 1.4 -- `Rvalue` sub-axes (45 rows)

**RV-002 Repeat**:

| Row ID | Construct | Predicate | Notes |
| ------ | --------- | --------- | ----- |
| RV-002 | `Rvalue::Repeat` | `RvalueVariant=Repeat` | `let _ = [0u32; 4];` |

**RV-005 CastKind sub-axes** (13 rows; F covered):

| Row ID | Construct | Predicate | Notes |
| ------ | --------- | --------- | ----- |
| RV-005-A | `PointerExposeProvenance` | `CastKind=PointerExposeProvenance` | `ptr.expose_provenance()` (nightly) |
| RV-005-B | `PointerWithExposedProvenance` | `CastKind=PointerWithExposedProvenance` | `ptr::with_exposed_provenance(addr)` |
| RV-005-C | `PointerCoercion::ReifyFnPointer` | `CastKind=PointerCoercion::ReifyFnPointer` | `let f: fn() = some_fn;` (dotted-compound F3 convention) |
| RV-005-D | `PointerCoercion::UnsafeFnPointer` | `CastKind=PointerCoercion::UnsafeFnPointer` | `let f: unsafe fn() = some_safe_fn;` |
| RV-005-E | `PointerCoercion::ClosureFnPointer` | `CastKind=PointerCoercion::ClosureFnPointer` | zero-capture closure -> fn pointer; LOW escalation risk because existing closure fixtures (`closure.rs`, `closure_call_sig_sync.rs`) avoid `Fn::call` successfully -- shape: `let f: fn() = (\|\| {}) as fn();` or implicit unsizing of closure to fn pointer |
| RV-005-G | `IntToInt` | `CastKind=IntToInt` | `let _: u64 = x as u64;` (x: u32) |
| RV-005-H | `FloatToInt` | `CastKind=FloatToInt` | `let _: u32 = x as u32;` (x: f32) |
| RV-005-I | `FloatToFloat` | `CastKind=FloatToFloat` | `let _: f64 = x as f64;` (x: f32) |
| RV-005-J | `IntToFloat` | `CastKind=IntToFloat` | `let _: f32 = x as f32;` (x: u32) |
| RV-005-K | `PtrToPtr` | `CastKind=PtrToPtr` | `let _: *mut u8 = ptr as *mut u8;` |
| RV-005-L | `FnPtrToPtr` | `CastKind=FnPtrToPtr` | `let _: *const () = fp as *const ();` |
| RV-005-M | `Transmute` | `CastKind=Transmute` | `unsafe { core::mem::transmute::<u32, i32>(x) }` |
| RV-005-N | `Subtype` | `CastKind=Subtype` | rare lifetime-subtype artefact; **likely escalates** |

**RV-006 BinOp sub-axes** (24 rows; A and I covered):

| Row ID | Op | Predicate | Snippet shape (canonical) |
| ------ | -- | --------- | ------------------------- |
| RV-006-B | `Sub` | `BinOp=Sub` | `pub fn f(x: u32, y: u32) -> u32 { x.wrapping_sub(y) }` (avoid Assert::Overflow) |
| RV-006-C | `Mul` | `BinOp=Mul` | similar w/ wrapping_mul |
| RV-006-D | `Div` | `BinOp=Div` | `pub fn f(x: u32, y: u32) -> u32 { x / y }` (Assert::DivisionByZero auto-emitted) |
| RV-006-E | `Rem` | `BinOp=Rem` | similar |
| RV-006-F | `AddUnchecked` | `BinOp=AddUnchecked` | `unsafe { x.unchecked_add(y) }` |
| RV-006-G | `SubUnchecked` | `BinOp=SubUnchecked` | `unsafe { x.unchecked_sub(y) }` |
| RV-006-H | `MulUnchecked` | `BinOp=MulUnchecked` | `unsafe { x.unchecked_mul(y) }` |
| RV-006-J | `SubWithOverflow` | `BinOp=SubWithOverflow` | **HIGH escalation risk under helper's overflow_checks=false**: ordinary signed `-` lowers to plain `BinOp::Sub` (not `SubWithOverflow`) per `crates/test-ir-fixtures/src/lib.rs:342-344` (overflow_checks disabled). Use explicit intrinsic: `#![feature(core_intrinsics)] unsafe { core::intrinsics::sub_with_overflow(x, y) }`. Hypothesis: the bridge maps MIR `SubWithOverflow` verbatim per `crates/regseq-driver/src/bridge/body/rvalue.rs:126-138`. |
| RV-006-K | `MulWithOverflow` | `BinOp=MulWithOverflow` | **HIGH escalation risk** -- same root cause. Use `core::intrinsics::mul_with_overflow(x, y)` with the same feature gate + intrinsic-emission hypothesis. |
| RV-006-L | `BitXor` | `BinOp=BitXor` | `x ^ y` |
| RV-006-M | `BitAnd` | `BinOp=BitAnd` | `x & y` |
| RV-006-N | `BitOr` | `BinOp=BitOr` | `x \| y` |
| RV-006-O | `Shl` | `BinOp=Shl` | `x << y` |
| RV-006-P | `ShlUnchecked` | `BinOp=ShlUnchecked` | `unsafe { x.unchecked_shl(y) }` |
| RV-006-Q | `Shr` | `BinOp=Shr` | `x >> y` |
| RV-006-R | `ShrUnchecked` | `BinOp=ShrUnchecked` | `unsafe { x.unchecked_shr(y) }` |
| RV-006-S | `Eq` | `BinOp=Eq` | `x == y` |
| RV-006-T | `Lt` | `BinOp=Lt` | `x < y` |
| RV-006-U | `Le` | `BinOp=Le` | `x <= y` |
| RV-006-V | `Ne` | `BinOp=Ne` | `x != y` |
| RV-006-W | `Ge` | `BinOp=Ge` | `x >= y` |
| RV-006-X | `Gt` | `BinOp=Gt` | `x > y` |
| RV-006-Y | `Cmp` | `BinOp=Cmp` | `x.cmp(&y)` (returns Ordering); rare three-way op |
| RV-006-Z | `Offset` | `BinOp=Offset` | `unsafe { ptr.offset(n) }` |

**RV-007 UnOp sub-axes** (3 rows):

| Row ID | Op | Predicate | Snippet shape |
| ------ | -- | --------- | ------------- |
| RV-007-A | `Not` | `UnOp=Not` | `pub fn f(b: bool) -> bool { !b }` |
| RV-007-B | `Neg` | `UnOp=Neg` | `pub fn f(x: i32) -> i32 { -x }` |
| RV-007-C | `PtrMetadata` | `UnOp=PtrMetadata` | `core::ptr::metadata(ptr)` |

**RV-009 AggregateKind sub-axes** (4 rows; C / D / F covered):

| Row ID | Construct | Predicate | Notes |
| ------ | --------- | --------- | ----- |
| RV-009-A | `Array` | `AggregateKind=Array` | `let _ = [1, 2, 3];` |
| RV-009-B | `Tuple` (non-unit) | `AggregateKind=Tuple` | `let _ = (1u32, 2u32);` |
| RV-009-E | `Union` | `AggregateKind=Union` | `pub union U { x: u32, y: f32 } let _ = U { x: 0 };` |
| RV-009-G | `RawPtr` | `AggregateKind=RawPtr` | raw-pointer aggregate from parts (slice / dyn from address+meta); **may escalate** if no clean source repro |

#### Section 1.5 -- `Statement` (2 rows)

| Row ID | Construct | Predicate | Notes |
| ------ | --------- | --------- | ----- |
| ST-003-A | `Assume` | `NonDivergingIntrinsic=Assume` | `unsafe { core::intrinsics::assume(cond); }` (requires `#![feature(core_intrinsics)]`) |
| ST-003-B | `CopyNonOverlapping` | `NonDivergingIntrinsic=CopyNonOverlapping` | `unsafe { core::ptr::copy_nonoverlapping(src, dst, n); }` |

#### Section 1.6 -- `Terminator` sub-axes (5 readily authorable rows)

| Row ID | Construct | Predicate | Notes |
| ------ | --------- | --------- | ----- |
| TM-004 | `Unreachable` (as sole terminator) | `TerminatorKind=Unreachable` | hard to author as the SOLE terminator of a block; possibly via `unreachable!()` macro |
| TM-006-A | `BoundsCheck` | `AssertKind=BoundsCheck` | `pub fn f(a: [u32; 4], i: usize) -> u32 { a[i] }` -- non-const index forces bounds check |
| TM-006-C | `OverflowNeg` | `AssertKind=OverflowNeg` | **HIGH escalation risk** -- `pub fn f(x: i32) -> i32 { -x }` runtime check on `i32::MIN` does NOT surface under helper's `overflow_checks=false` per `crates/test-ir-fixtures/src/lib.rs:342-344`. Use explicit intrinsic or document as escalation. |
| TM-006-D | `DivisionByZero` | `AssertKind=DivisionByZero` | `x / y` runtime check |
| TM-006-E | `RemainderByZero` | `AssertKind=RemainderByZero` | `x % y` runtime check |

TM-006-F (MisalignedPointerDereference), TM-006-G
(NullPointerDereference), TM-006-H (InvalidEnumConstruction)
are deferred from F4: no clean source repro that reliably trips
the runtime check. Escalation-by-default; document in
expectations file.

#### Section 2.3 -- `FFlavor` (4 rows)

| Row ID | Construct | Predicate | Notes |
| ------ | --------- | --------- | ----- |
| FF-004 | `ReifyShim` | `FunctionInstanceFlavor=ReifyShim` | `pub fn target() {} pub fn entry() -> fn() { target }` -- let-binding reifies via shim |
| FF-005 | `FnPtrShim` | `FunctionInstanceFlavor=FnPtrShim` | `pub fn target() {} pub fn entry() { let fp: fn() = target; fp(); }` -- fn-pointer call site through a let-bound fn pointer surfaces an FnPtrShim instance |
| FF-012 | `CloneShim` | `FunctionInstanceFlavor=CloneShim` | `#[derive(Clone, Copy)] struct S(u32); pub fn f(s: S) -> S { s.clone() }` |
| FF-013 | `FnPtrAddrShim` | `FunctionInstanceFlavor=FnPtrAddrShim` | `pub fn target() {} pub fn entry() -> usize { (target as fn() as *const ()) as usize }` |

FF-007 (ClosureOnceShim) remains blocked by IX-014's escalation
family.

#### Section 3 -- source-level features (3 rows)

| Row ID | Construct | Predicate | Notes |
| ------ | --------- | --------- | ----- |
| FT-002 | Generic Drop impl | `All([ItemKind=Struct, FunctionInstanceFlavor=DropGlue])` (loose) | `pub struct W<T> { v: T } impl<T> Drop for W<T> { fn drop(&mut self) {} } let _w = W { v: 0u32 };` |
| FT-005 | Explicit `core::mem::drop(x)` | `CallCallee=Direct` (loose) | `pub struct R; impl Drop for R { fn drop(&mut self) {} } pub fn f() { let r = R; core::mem::drop(r); }` |
| FT-008 | Trait default + impl override | `ItemKind=Function` (loose) | trait with default body, impl overrides |

FT-003 covered indirectly by IX-012. FT-004 deferred (no
negative-assertion predicate in F3 dispatch). FT-009 covered by
IX-001. FT-011-014 blocked by IX-004/IX-010 family. FT-016
covered by RJ-001's rejected counterpart (out of F4 scope as
documented). FT-017 / FT-019 / FT-020 / FT-021 already covered.

#### Section 4 -- compilation/toolchain (1 row)

| Row ID | Construct | Predicate | Notes |
| ------ | --------- | --------- | ----- |
| CT-011 | N>1 generic args | `ItemKind=Struct` (loose; F3 has no generic-args-width predicate) | `pub struct W<T, U> { a: T, b: U } pub fn f() -> W<u32, u32> { W { a: 0, b: 0 } }` |

CT-010 (`Linkage` enum sub-axes) is deferred from F4: the bridge
sets only `Default` / `NoMangle` / `Foreign` on rendered items
per `crates/regseq-driver/src/bridge/mod.rs:908-955`; there is
no current producer for `Linkage=Weak`. Once a bridge-side
`Weak` producer lands (separate task), CT-010 becomes
authorable.

#### Section 5 -- interaction re-attempt (1 row)

| Row ID | Construct | Predicate | Notes |
| ------ | --------- | --------- | ----- |
| IX-017 | Diag-log intrinsic x generic caller | `BodyAbsent` | re-attempt of F2-escalated IX-017 per F3 expectations file's B6 probe outcome=YES. Uses the F2 round-2 snippet shape (inlined log shim + `regseq::log::info!("event")` with no placeholders) per `.harness/expectations/regseq-driver-65.md:259-281`. |

#### Section 6 -- rejection rows (10 rows)

| Row ID | Construct | Expected rejection | Notes |
| ------ | --------- | ------------------ | ----- |
| RJ-006 | `Statement` (unsupported MIR statement) | `DslRejection` with `kind = "Statement"` | rare; specific MIR statement kinds are bridge-rejected. May need a niche source shape. |
| RJ-015 | `Foreign` (non-extern-block foreign type) | `Foreign` | `#![feature(extern_types)] extern { type Opaque; }` + use site |
| RJ-024 | `UnsupportedEnumLayout` | `UnsupportedEnumLayout` | enum with `#[repr(C, packed)]` or unusual tag layout that the bridge can't compute |
| RJ-031-D | `DiagLogFormat::NonSubsetModifier` | `DiagLogFormat` with `reason = "NonSubsetModifier"` | inlined log shim + `regseq::log::info!("x={:b}", x)` (binary modifier outside the hex subset per spec.md §26.7) |
| RJ-032-A | `DiagLogValidation::D1` | `DiagLogValidation` with `diagnostic = "D1"` | inlined log shim + arity-mismatched call: 2 placeholders, 1 arg |
| RJ-032-B | `DiagLogValidation::D2` | `DiagLogValidation` with `diagnostic = "D2"` | inlined log shim + modifier-type mismatch (e.g., `{:x}` on `&str`) |
| RJ-032-C | `DiagLogValidation::D4` | `DiagLogValidation` with `diagnostic = "D4"` | inlined log shim + non-`LogArg`-admissible operand |
| RJ-032-D | `DiagLogValidation::D6` | `DiagLogValidation` with `diagnostic = "D6"` | `let s: &str = "..."; info!(s);` -- args[0] not a literal |
| RJ-032-E | `DiagLogValidation::D7` | `DiagLogValidation` with `diagnostic = "D7"` | format template ending in `\n` |
| RJ-032-F | `DiagLogValidation::D8` | `DiagLogValidation` with `diagnostic = "D8"` | too many `&str` operands for the target binding's `max_str_args_per_call` |

### Change B: per-snippet authoring + verification loop (per F2/F3 protocol)

For each candidate row above:

1. Author candidate snippet at
   `crates/bridge-coverage-runner/snippets/<row_id>.rs`.
2. Append the row's `[[row]]` entry to
   `crates/bridge-coverage-runner/matrix_metadata/rows.toml`
   with explicit `cluster_key` (top-level row ID or parent
   for sub-axis rows).
3. Run the runner:
   ```bash
   target/debug/bridge-coverage-runner \
     --metadata crates/bridge-coverage-runner/matrix_metadata/rows.toml \
     --snippets crates/bridge-coverage-runner/snippets/ \
     --matrix-md docs/discussions/regseq-model/bridge-coverage-matrix.md \
     --out /tmp/brcr-f4/
   ```
4. Inspect `/tmp/brcr-f4/report.md`:
   - Positive row: `Ok` + inspection-pass -> accept.
   - Rejected row: `Rejected(variant)` + sub-axis-match -> accept.
   - Otherwise: refine snippet (different feature gate, different
     shape, different forcing site) and retry.
5. Max 3 retries per row. After 3 retries WITHOUT a passing
   candidate:
   - Snippet file removed.
   - Sidecar entry NOT committed.
   - Row documented in expectations file as escalated.
6. Commit incrementally (per-row OR per coherent cluster). The
   runner's drift check applies at every commit boundary.

The implementer is encouraged to land in topical commits
(e.g., "Section 1.4 RV-006 BinOp sub-axes") to keep the
git-log readable; the final F4 commit count is the
implementer's call.

### Change C: harness expectations entry

**New file**: `.harness/expectations/regseq-driver-67.md`.

Record:

- Per-row PASS / ESCALATED outcome for every F4 candidate from
  Change A. Use a per-section table mirroring the spec.
- Aggregate counts: K candidates targeted, K' committed, K-K'
  escalated.
- For each escalation: candidate shapes attempted, empirical
  outcomes, follow-up direction.
- IX-017 re-attempt outcome (PASS / FAIL / refinement notes).
- Final runner Mode A report summary
  (`Total rows`, `Pass`, `Fail`, `Outcomes by class`).
- Strategy-deviation record (NONE expected for F4 -- the
  authoring protocol matches strategy §5 exactly).
- Post-F4 readiness signal: which uncovered matrix rows
  REMAIN after F4 (the blocked-* set + any F4 escalations).
  This is the input to the post-F4 batch-triage task.

## Non-Goals

- Do NOT add new `InspectionPredicate` enum variants. F4 uses
  the existing 22-variant vocabulary (with the F3 dispatch).
- Do NOT modify the F1+F2+F3 runner architecture beyond
  appending sidecar rows + snippet files + the harness
  expectations file. `inspect.rs`, `matrix.rs`, `snippet.rs`,
  `oracle.rs`, `cluster.rs`, `report.rs`, `main.rs`,
  `build.rs`, `Cargo.toml` STAY untouched.
- Do NOT modify the F0 helper at
  `crates/test-ir-fixtures/src/lib.rs`.
- Do NOT modify spec.md, pretty-printer.md, generic-c-backend.md,
  bridge-coverage-matrix.md, or
  bridge-coverage-runner-strategy.md. (The matrix doc's
  `status = "uncovered"` markings will become inaccurate after
  F4; updating the matrix is a separate documentation task.)
- Do NOT attempt the out-of-scope rows enumerated in the
  Description. Specifically: blocked-rustc-infeasible (RJ-014 /
  RJ-017 / RJ-020 / RJ-021), blocked-bridge-rejects-by-design
  with no positive-coverage shape (FF-008 through FF-015),
  blocked-open-bridge-bug (IX-004 / IX-010 / FT-011 / IX-014),
  Section 7 validator-invariant rows, Section 2.2 closure-call
  family blocked rows (SK-004 / SK-005), Section 3 closure
  blocked rows (FT-012 / FT-013 / FT-014), Section 4
  cross-edition / alloc-dep / multi-crate rows (CT-004 / CT-007
  / CT-008), and Section 6 hard-to-author rejections (RJ-001 /
  RJ-009-RJ-013 / RJ-018 / RJ-023 / RJ-025 / RJ-029 / RJ-030).
- Do NOT exceed the 3-retry budget per row. Rows that fail
  their budget escalate.
- Do NOT add new dependencies.
- Do NOT modify `tests/fixtures/playground/`.
- Do NOT file investigation tasks for F4-surfaced findings
  during F4 itself. Batch triage runs AFTER F4 close (separate
  task per strategy §10 Mode B).
- Do NOT modify the F3 sub-axis matcher or its uplifted rows.
  Any new F4 rejection row uses the appropriate sub-axis field
  values directly.
- Do NOT add JSON report output, CI gating, or Mode B agent
  integration (deferred to F5+).
- Do NOT re-attempt IX-014 in F4. IX-014 stays escalated
  against the IX-004/IX-010 OPEN BUG family.

## Test Plan

### B1: workspace stays green

`cargo test --workspace -- --test-threads=1` green:

- All existing F0/F1/F2/F3 tests pass unchanged.
- The 3 F1 smoke tests at
  `crates/bridge-coverage-runner/tests/smoke.rs` continue to
  pass (the `smoke_full_demo_set_runs_to_completion` test
  asserts `Total == Pass` and `Fail == 0`; F4's monotonic
  sidecar growth preserves the invariant).

### B2: runner reports all-pass on the expanded sidecar

After every commit boundary AND after F4 close, the manual
Mode A invocation produces a `report.md` with `Total rows =
32 + K` (where K is the count of committed F4 rows),
`Pass = Total rows`, `Fail = 0`, exit code 0. No row regression.

### B3: per-row outcomes recorded

The harness expectations file at
`.harness/expectations/regseq-driver-67.md` records per-row
PASS / ESCALATED status with per-section tables, plus aggregate
counts and the post-F4 readiness signal.

### B4: `cargo fmt` + `cargo clippy` clean

`cargo fmt --all -- --check` and `cargo clippy --workspace
--all-targets -- -D warnings` clean. F4 adds no new code
modules; only new snippets + sidecar entries + the expectations
file.

### B5: F0 / F1 / F2 / F3 architecture unchanged

`git diff main -- crates/test-ir-fixtures/` empty. `git diff main --
crates/bridge-coverage-runner/src/` empty (no `inspect.rs`,
`main.rs`, `matrix.rs`, etc. edits in F4). `git diff main --
crates/bridge-coverage-runner/Cargo.toml
crates/bridge-coverage-runner/build.rs
crates/bridge-coverage-runner/tests/smoke.rs
crates/bridge-coverage-runner/snippets/README.md` empty.

### B6: drift check holds at every commit boundary

The runner's drift check at `matrix.rs::check_drift` is
invoked at every Mode A run; any commit that introduces a
sidecar row id NOT present in the matrix would hard-fail.
F4's row IDs all derive from the matrix doc, so drift never
fires; documented invariant.

## Review

### Files to send to Codex (`scripts/python/codex_runner.py`)

- This file
  (`.harness/tasks/regseq-driver/67-bridge-coverage-runner-f4-full-corpus.md`).
- The runner strategy doc
  (`docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`)
  for §5, §10, §11 cross-reference.
- The matrix doc
  (`docs/discussions/regseq-model/bridge-coverage-matrix.md`)
  for row-id + construct verification per the F4 row tables.

### Reviewer focus

- **Row-set completeness**: do the per-section tables cover
  every authorable uncovered matrix row? Any rows missing or
  spuriously included? Cross-reference matrix doc sections
  1.1 / 1.2 / 1.3 / 1.4 / 1.5 / 1.6 / 2.3 / 3 / 4 / 5 / 6.
- **Predicate assignment correctness**: for each positive row,
  does the assigned predicate match the row's claimed construct
  per the F3 dispatch surface at
  `crates/bridge-coverage-runner/src/inspect.rs`?
- **Sub-axis correctness for rejection rows**: do the
  `expected_rejection.variant` + sub-axis field combinations
  match the actual `BridgeError` variant per
  `crates/regseq-driver/src/bridge/mod.rs`?
- **Loose-predicate disclosures**: rows using `TyVariant=Dynamic`
  for auto-trait combos, `ItemKind=Struct` for N>1 generic args,
  etc., must be explicitly marked as loose proxies.
- **Out-of-scope rationale**: are the documented blocks (per the
  Description's enumeration) accurately tied to matrix
  classification (`blocked-*` markings, OPEN BUG family,
  feature-gate complexity)?
- **Escalation-risk flags**: which rows are flagged as "may
  escalate" with what rationale? Are any high-risk rows
  inappropriately flagged as low-risk (or vice versa)?
- **Authoring protocol clarity**: does Change B describe the
  per-row author/verify/retry loop concretely enough for the
  implementer agent?
- **Non-Goals discipline**: F4 enumerates ~14 Non-Goal
  categories. Are any Implementation Notes accidentally
  creeping into a Non-Goal?
- **IX-017 re-attempt setup**: does Section 5's IX-017 entry
  reference the F3 expectations file's B6 probe outcome and
  the F2 round-2 snippet shape correctly?
- **Citation drift**: every file:line citation against HEAD.

### Adversarial prompt template

```
<task>
Review .harness/tasks/regseq-driver/67-bridge-coverage-runner-f4-full-corpus.md.

Identify:
- Row-set completeness across matrix sections 1.1 / 1.2 / 1.3 / 1.4 / 1.5 / 1.6 / 2.3 / 3 / 4 / 5 / 6.
- Per-row predicate assignment correctness vs F3 dispatch surface.
- Sub-axis correctness for rejection rows (variant + kind/position/reason/diagnostic).
- Loose-predicate disclosures.
- Out-of-scope rationale (blocked-* / OPEN BUG family / feature-gate complexity).
- Escalation-risk flags accuracy.
- Authoring protocol clarity.
- Non-Goals discipline.
- IX-017 re-attempt setup.
- Citation drift against HEAD.
</task>

<grounding_rules>
- Cite file:line for every claim.
- Mark hypotheses as hypotheses.
- For predicate assignment, inspect crates/bridge-coverage-runner/src/inspect.rs for the implemented dispatch.
- For expected_rejection sub-axes, inspect crates/regseq-driver/src/bridge/mod.rs for BridgeError variant + sub-enum spellings.
- For row-id existence in the matrix, inspect docs/discussions/regseq-model/bridge-coverage-matrix.md.
- For escalation risk, prior empirical evidence in .harness/expectations/regseq-driver-65.md and .harness/expectations/regseq-driver-66.md is authoritative.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
ROW-SET COMPLETENESS CHECK: PASS / FAIL with per-section status
PREDICATE ASSIGNMENT CHECK: PASS / FAIL with per-row issues
SUB-AXIS CORRECTNESS CHECK: PASS / FAIL with per-row issues
LOOSE-PREDICATE DISCLOSURES: PASS / FAIL
OUT-OF-SCOPE RATIONALE: PASS / FAIL
ESCALATION-RISK FLAGS: PASS / FAIL with per-row commentary
AUTHORING PROTOCOL CLARITY: PASS / FAIL
NON-GOALS DISCIPLINE: PASS / FAIL
IX-017 SETUP: PASS / FAIL with citation
CITATION DRIFT: list or "none"
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
- `cargo test --workspace -- --test-threads=1` green; no
  regressions to F0/F1/F2/F3 tests; smoke tests pass.
- Manual Mode A invocation per B2 produces `report.md` with
  `Total rows == Pass` and `Fail = 0` after F4 close.
- `git diff --stat main..` shows changes confined to:
  - `crates/bridge-coverage-runner/matrix_metadata/rows.toml`
    (Change A: per-row sidecar entries appended)
  - `crates/bridge-coverage-runner/snippets/<row_id>.rs`
    (Change A: new snippet files per committed row)
  - `.harness/expectations/regseq-driver-67.md` (Change C)
  - `.harness/tasks.json` (registration entry)
- No F0 modifications.
- No F1/F2/F3 source module modifications (`inspect.rs` /
  `main.rs` / `matrix.rs` / `snippet.rs` / `oracle.rs` /
  `cluster.rs` / `report.rs` / `build.rs` / `Cargo.toml`).
- No `tests/smoke.rs` or `snippets/README.md` modifications
  (architecturally part of F1+F3).
- No bridge / model / spec / pretty-printer / generic-c-backend
  / matrix / strategy doc changes.
- Codex Task Review verdict: APPROVED or APPROVED_WITH_MINOR_FIXES.

## Acceptance Criteria

1. **F4 candidate rows attempted**: the implementer attempts
   every row enumerated in Change A's per-section tables under
   the 3-retry authoring protocol. No row is silently skipped;
   any row not committed is documented as escalated in the
   expectations file with candidate shapes attempted.

2. **K committed sidecar rows land** (K <= ~102):
   `crates/bridge-coverage-runner/matrix_metadata/rows.toml`
   contains `32 + K` rows total (10 F1 + 22 F2 + K F4 committed).
   Every row carries explicit `cluster_key`.

3. **K committed snippets land**:
   `crates/bridge-coverage-runner/snippets/<row_id>.rs` exists
   for every committed F4 row (no orphan sidecar entries; no
   orphan snippet files).

4. **Runner reports all-pass after F4 close**: manual Mode A
   per B2 produces exit code 0, `Total rows = 32 + K`, `Pass =
   Total rows`, `Fail = 0`.

5. **IX-017 re-attempt outcome documented**: IX-017 is
   re-authored with the F2 round-2 snippet shape + the new
   `BodyAbsent` predicate. Outcome (PASS / FAIL with detail /
   ESCALATED) recorded in the expectations file.

6. **Per-row expectations recorded**: the harness expectations
   file at `.harness/expectations/regseq-driver-67.md` carries
   per-section per-row tables matching Change A's enumeration,
   with final predicate / final snippet path / actual outcome /
   PASS-or-ESCALATED status for every attempted row.

7. **Escalation log complete**: every escalated row is
   documented with candidate shapes attempted, empirical
   rejection reasons at each attempt, and a proposed follow-up
   direction (refine shape with more context, file a separate
   bug-discovery task, defer to a future task).

8. **Post-F4 readiness signal**: the expectations file's
   closing note enumerates the matrix rows that REMAIN
   uncovered post-F4: the documented blocked-* set + any F4
   escalations. This list is the canonical input to the
   post-F4 batch-triage task (per strategy §10 Mode B).

9. **F0/F1/F2/F3 architecture untouched**: `git diff main --
   crates/test-ir-fixtures/ crates/bridge-coverage-runner/src/
   crates/bridge-coverage-runner/build.rs
   crates/bridge-coverage-runner/Cargo.toml
   crates/bridge-coverage-runner/tests/
   crates/bridge-coverage-runner/snippets/README.md` returns
   zero.

10. **No scope creep**: `git diff main` touches ONLY the files
    listed in Quality Gate's `git diff --stat` enumeration. No
    bridge / model / spec / pretty-printer / generic-c-backend
    / matrix / strategy doc changes.

11. **Drift check holds**: at every commit boundary, the
    runner's drift check at
    `crates/bridge-coverage-runner/src/matrix.rs::check_drift`
    passes (no committed sidecar row ID is absent from the
    matrix doc).

12. **Post-F4 milestone closed**: with F4 close, the runner's
    sidecar covers every empirically-authorable uncovered
    matrix row (within the F4 escalation filter). The
    bridge-coverage-runner's primary deliverable -- the
    discovery engine that turns the matrix into a deterministic
    triage queue -- is now operationally complete. The next
    step in the bridge-coverage strategy is the post-F4 batch
    triage task (separate from F4).
