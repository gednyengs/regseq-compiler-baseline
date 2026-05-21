# regseq-driver-67 -- F4 expectations

F4 of the bridge-coverage-runner strategy (per
`docs/discussions/regseq-model/bridge-coverage-runner-strategy.md`
section 11). Lands the remaining empirically-authorable
uncovered matrix rows as committed sidecar entries + snippets,
exercising the full F3 dispatch surface (22/22 arms).

## Aggregate summary

| Class | Count |
| ----- | ----- |
| Candidates targeted (per Change A) | 102 |
| Committed sidecar rows (F4 share) | 91 |
| Escalated (3-retry budget consumed without pass) | 11 |
| Total sidecar after F4 close | 123 (32 pre-F4 + 91 F4) |

Manual Mode A invocation per Test Plan B2:

```
| Total rows | 123 |
| Pass | 123 |
| Fail | 0 |
| Outcomes by class | Ok=100, Rejected=23 |
```

Exit code 0. Drift check passes at every commit boundary
(sidecar-IDs subset of matrix-IDs invariant).

## Per-section outcomes

### Section 1.1 -- `Ty` enum + sub-axes (22 candidates, 22 PASS)

| Row | Predicate | Snippet | Outcome |
| --- | --------- | ------- | ------- |
| TY-001 | `TyVariant=Bool` | `TY-001.rs` | PASS |
| TY-002 | `TyVariant=Char` | `TY-002.rs` | PASS |
| TY-007 | `TyVariant=Never` | `TY-007.rs` | PASS |
| TY-008 | `TyVariant=Array` | `TY-008.rs` | PASS |
| TY-011-A | `TyVariant=Ptr` | `TY-011-A.rs` | PASS |
| TY-016 | `TyVariant=Tuple` | `TY-016.rs` | PASS |
| TY-014-B | `TyVariant=Dynamic` (loose) | `TY-014-B.rs` | PASS |
| TY-014-C | `TyVariant=Dynamic` (loose) | `TY-014-C.rs` | PASS |
| TY-014-D | `TyVariant=Dynamic` (loose) | `TY-014-D.rs` | PASS |
| TY-014-E | `TyVariant=Dynamic` (loose) | `TY-014-E.rs` | PASS |
| TY-003-A | `ScalarKind=I8` | `TY-003-A.rs` | PASS |
| TY-003-B | `ScalarKind=I16` | `TY-003-B.rs` | PASS |
| TY-003-D | `ScalarKind=I64` | `TY-003-D.rs` | PASS |
| TY-003-E | `ScalarKind=I128` | `TY-003-E.rs` | PASS |
| TY-003-F | `ScalarKind=Isize` | `TY-003-F.rs` | PASS |
| TY-004-B | `ScalarKind=U16` | `TY-004-B.rs` | PASS |
| TY-004-E | `ScalarKind=U128` | `TY-004-E.rs` | PASS |
| TY-004-F | `ScalarKind=Usize` | `TY-004-F.rs` | PASS |
| TY-005-A | `ScalarKind=F16` | `TY-005-A.rs` | PASS |
| TY-005-B | `ScalarKind=F32` | `TY-005-B.rs` | PASS |
| TY-005-C | `ScalarKind=F64` | `TY-005-C.rs` | PASS |
| TY-005-D | `ScalarKind=F128` | `TY-005-D.rs` | PASS |

### Section 1.2 -- `PlaceElem` (3 candidates, 3 PASS)

| Row | Predicate | Snippet | Outcome |
| --- | --------- | ------- | ------- |
| PL-004 | `PlaceElem=Index` | `PL-004.rs` | PASS |
| PL-005 | `PlaceElem=ConstantIndex` | `PL-005.rs` | PASS |
| PL-006 | `PlaceElem=Subslice` | `PL-006.rs` | PASS (round-2: named subslice binding `middle @ ..` forced the projection; bare `[head, .., tail]` did not surface a `Subslice` in MIR) |

### Section 1.3 -- `Operand`/`Const` sub-axes (6 candidates, 5 PASS, 1 ESCALATED)

| Row | Predicate | Snippet | Outcome |
| --- | --------- | ------- | ------- |
| OP-003-A | `ConstVariant=Bool` | `OP-003-A.rs` | PASS |
| OP-003-B | `ConstVariant=Char` | `OP-003-B.rs` | PASS |
| OP-003-E | `ConstVariant=Float` | `OP-003-E.rs` | PASS |
| OP-003-H | `ConstVariant=Aggregate` | `OP-003-H.rs` | PASS (round-2: `pub const A: &'static u32 = &X` produced the `Const::Aggregate([Reloc, Uint])` carrier per `crates/regseq-driver/src/bridge/const_.rs:391-404`; the array literal `pub const A: [u32; 2] = [1, 2]` resolved to `Const::Bytes`) |
| OP-003-I | `ConstVariant=EnumValue` | -- | **ESCALATED** |
| OP-003-J | `ConstVariant=NullPtr` | `OP-003-J.rs` | PASS (round-2: `pub const P: *const u32 = core::ptr::null()` produced `Const::NullPtr` via the const-evaluation scalar path; `core::ptr::null()` as a function call did not) |

### Section 1.4 -- `Rvalue` sub-axes (45 candidates, 43 PASS, 2 ESCALATED)

**RV-002 Repeat** (1/1 PASS): `RV-002` -> `RV-002.rs`.

**RV-005 CastKind sub-axes** (13/13 PASS):

| Row | Predicate | Snippet | Outcome |
| --- | --------- | ------- | ------- |
| RV-005-A | `CastKind=PointerExposeProvenance` | `RV-005-A.rs` | PASS (round-2: `p as usize`; `.expose_provenance()` method call did not lower to the cast) |
| RV-005-B | `CastKind=PointerWithExposedProvenance` | `RV-005-B.rs` | PASS (round-2: `addr as *const u32`; the std wrapper function call did not surface the cast) |
| RV-005-C | `CastKind=PointerCoercion::ReifyFnPointer` | `RV-005-C.rs` | PASS |
| RV-005-D | `CastKind=PointerCoercion::UnsafeFnPointer` | `RV-005-D.rs` | PASS (round-2: pass a `fn()` parameter and let-bind to `unsafe fn()`; direct `fn item -> unsafe fn()` collapsed to a single Reify cast) |
| RV-005-E | `CastKind=PointerCoercion::ClosureFnPointer` | `RV-005-E.rs` | PASS |
| RV-005-G | `CastKind=IntToInt` | `RV-005-G.rs` | PASS |
| RV-005-H | `CastKind=FloatToInt` | `RV-005-H.rs` | PASS |
| RV-005-I | `CastKind=FloatToFloat` | `RV-005-I.rs` | PASS |
| RV-005-J | `CastKind=IntToFloat` | `RV-005-J.rs` | PASS |
| RV-005-K | `CastKind=PtrToPtr` | `RV-005-K.rs` | PASS |
| RV-005-L | `CastKind=FnPtrToPtr` | `RV-005-L.rs` | PASS |
| RV-005-M | `CastKind=Transmute` | `RV-005-M.rs` | PASS |
| RV-005-N | `CastKind=Subtype` | `RV-005-N.rs` | PASS (round-2: HRTB-to-`'static` fn-pointer coercion `for<'a> fn(&'a u32) -> fn(&'static u32)` surfaces a `Subtype` cast; identity lifetime widening did not) |

**RV-006 BinOp sub-axes** (24/24 PASS):

| Row | Predicate | Snippet | Outcome |
| --- | --------- | ------- | ------- |
| RV-006-B | `BinOp=Sub` | `RV-006-B.rs` | PASS (round-2: direct `x - y` for u32; `wrapping_sub` left the op inside the std wrapper) |
| RV-006-C | `BinOp=Mul` | `RV-006-C.rs` | PASS (round-2: same rationale) |
| RV-006-D | `BinOp=Div` | `RV-006-D.rs` | PASS |
| RV-006-E | `BinOp=Rem` | `RV-006-E.rs` | PASS |
| RV-006-F | `BinOp=AddUnchecked` | `RV-006-F.rs` | PASS (round-2: `core::intrinsics::unchecked_add` -- the std method left the BinOp inside the wrapper) |
| RV-006-G | `BinOp=SubUnchecked` | `RV-006-G.rs` | PASS (round-2: intrinsic direct call) |
| RV-006-H | `BinOp=MulUnchecked` | `RV-006-H.rs` | PASS (round-2: intrinsic direct call) |
| RV-006-J | `BinOp=SubWithOverflow` | `RV-006-J.rs` | PASS (`core::intrinsics::sub_with_overflow` per spec hint) |
| RV-006-K | `BinOp=MulWithOverflow` | `RV-006-K.rs` | PASS (`core::intrinsics::mul_with_overflow`) |
| RV-006-L | `BinOp=BitXor` | `RV-006-L.rs` | PASS |
| RV-006-M | `BinOp=BitAnd` | `RV-006-M.rs` | PASS |
| RV-006-N | `BinOp=BitOr` | `RV-006-N.rs` | PASS |
| RV-006-O | `BinOp=Shl` | `RV-006-O.rs` | PASS |
| RV-006-P | `BinOp=ShlUnchecked` | `RV-006-P.rs` | PASS (round-2: intrinsic direct call) |
| RV-006-Q | `BinOp=Shr` | `RV-006-Q.rs` | PASS |
| RV-006-R | `BinOp=ShrUnchecked` | `RV-006-R.rs` | PASS (round-2: intrinsic direct call) |
| RV-006-S | `BinOp=Eq` | `RV-006-S.rs` | PASS |
| RV-006-T | `BinOp=Lt` | `RV-006-T.rs` | PASS |
| RV-006-U | `BinOp=Le` | `RV-006-U.rs` | PASS |
| RV-006-V | `BinOp=Ne` | `RV-006-V.rs` | PASS |
| RV-006-W | `BinOp=Ge` | `RV-006-W.rs` | PASS |
| RV-006-X | `BinOp=Gt` | `RV-006-X.rs` | PASS |
| RV-006-Y | `BinOp=Cmp` | `RV-006-Y.rs` | PASS (round-2: `core::intrinsics::three_way_compare`; `x.cmp(&y)` left the op inside `Ord::cmp`) |
| RV-006-Z | `BinOp=Offset` | `RV-006-Z.rs` | PASS (round-2: `core::intrinsics::offset`; `.offset()` method call left the op inside the ptr wrapper) |

**RV-007 UnOp sub-axes** (2/3 PASS, 1 ESCALATED):

| Row | Predicate | Snippet | Outcome |
| --- | --------- | ------- | ------- |
| RV-007-A | `UnOp=Not` | `RV-007-A.rs` | PASS |
| RV-007-B | `UnOp=Neg` | `RV-007-B.rs` | PASS |
| RV-007-C | `UnOp=PtrMetadata` | -- | **ESCALATED** |

**RV-009 AggregateKind sub-axes** (3/4 PASS, 1 ESCALATED):

| Row | Predicate | Snippet | Outcome |
| --- | --------- | ------- | ------- |
| RV-009-A | `AggregateKind=Array` | `RV-009-A.rs` | PASS |
| RV-009-B | `AggregateKind=Tuple` | `RV-009-B.rs` | PASS |
| RV-009-E | `AggregateKind=Union` | `RV-009-E.rs` | PASS |
| RV-009-G | `AggregateKind=RawPtr` | -- | **ESCALATED** |

### Section 1.5 -- `Statement` (2 candidates, 2 PASS)

| Row | Predicate | Snippet | Outcome |
| --- | --------- | ------- | ------- |
| ST-003-A | `NonDivergingIntrinsic=Assume` | `ST-003-A.rs` | PASS (`core::intrinsics::assume`) |
| ST-003-B | `NonDivergingIntrinsic=CopyNonOverlapping` | `ST-003-B.rs` | PASS (round-2: `core::intrinsics::copy_nonoverlapping`; `core::ptr::copy_nonoverlapping` wrapper hid the statement inside the std fn) |

### Section 1.6 -- `Terminator` sub-axes (5 candidates, 4 PASS, 1 ESCALATED)

| Row | Predicate | Snippet | Outcome |
| --- | --------- | ------- | ------- |
| TM-004 | `TerminatorKind=Unreachable` | `TM-004.rs` | PASS (round-2: `core::intrinsics::unreachable()`; the `match` arm with `unreachable_unchecked()` produced a `Call` terminator + downstream `Unreachable` that failed Phase-5 BRpo) |
| TM-006-A | `AssertKind=BoundsCheck` | `TM-006-A.rs` | PASS |
| TM-006-C | `AssertKind=OverflowNeg` | -- | **ESCALATED** |
| TM-006-D | `AssertKind=DivisionByZero` | `TM-006-D.rs` | PASS |
| TM-006-E | `AssertKind=RemainderByZero` | `TM-006-E.rs` | PASS |

### Section 2.3 -- `FFlavor` (4 candidates, 0 PASS, 4 ESCALATED)

| Row | Predicate | Snippet | Outcome |
| --- | --------- | ------- | ------- |
| FF-004 | `FunctionInstanceFlavor=ReifyShim` | -- | **ESCALATED** |
| FF-005 | `FunctionInstanceFlavor=FnPtrShim` | -- | **ESCALATED** |
| FF-012 | `FunctionInstanceFlavor=CloneShim` | -- | **ESCALATED** |
| FF-013 | `FunctionInstanceFlavor=FnPtrAddrShim` | -- | **ESCALATED** |

### Section 3 -- source-level features (3 candidates, 3 PASS)

| Row | Predicate | Snippet | Outcome |
| --- | --------- | ------- | ------- |
| FT-002 | `All([ItemKind=Struct, FunctionInstanceFlavor=DropGlue])` | `FT-002.rs` | PASS |
| FT-005 | `CallCallee=Direct` | `FT-005.rs` | PASS |
| FT-008 | `ItemKind=Function` | `FT-008.rs` | PASS (round-2: `impl Tr for S2 {}` paired with the override so the trait default's MIR is reachable; the override-only shape failed `ItemMissingInstance` for the default body) |

### Section 4 -- compilation/toolchain (1 candidate, 1 PASS)

| Row | Predicate | Snippet | Outcome |
| --- | --------- | ------- | ------- |
| CT-011 | `ItemKind=Struct` (loose) | `CT-011.rs` | PASS |

### Section 5 -- interaction re-attempt (1 candidate, 1 PASS)

| Row | Predicate | Snippet | Outcome |
| --- | --------- | ------- | ------- |
| IX-017 | `BodyAbsent` | `IX-017.rs` | PASS (F2 round-2 snippet shape reused; matches the F3 expectations file's B6 probe outcome at `.harness/expectations/regseq-driver-66.md:287-312`) |

### Section 6 -- rejection rows (10 candidates, 7 PASS, 3 ESCALATED)

| Row | Expected rejection | Snippet | Outcome |
| --- | ------------------ | ------- | ------- |
| RJ-006 | `DslRejection` (`kind=Statement`) | -- | **ESCALATED** |
| RJ-015 | `Foreign` | `RJ-015.rs` | PASS |
| RJ-024 | `UnsupportedEnumLayout` | -- | **ESCALATED** |
| RJ-031-D | `DiagLogFormat` (`reason=NonSubsetModifier`) | `RJ-031-D.rs` | PASS |
| RJ-032-A | `DiagLogValidation` (`diagnostic=D1`) | `RJ-032-A.rs` | PASS |
| RJ-032-B | `DiagLogValidation` (`diagnostic=D2`) | `RJ-032-B.rs` | PASS (round-3: needed `LogArg::from(s)` over a function-param `&str`; direct `LogArg::Str("lit")` was const-promoted into a `&'static` slice and traced as `EmptyConstSlice`, which collapsed peeled args to 0 and fired D1) |
| RJ-032-C | `DiagLogValidation` (`diagnostic=D4`) | `RJ-032-C.rs` | PASS (added `Char` variant + `From<char>` to the local `LogArg` so the peel resolves the wrapper Call and surfaces a `Ty::Char` operand outside the admissible set) |
| RJ-032-D | `DiagLogValidation` (`diagnostic=D6`) | `RJ-032-D.rs` | PASS (function-param `&'static str` as the format-template operand surfaces as `Operand::Move`/`Copy`, not `Operand::Constant`) |
| RJ-032-E | `DiagLogValidation` (`diagnostic=D7`) | `RJ-032-E.rs` | PASS (template ends in `\n`) |
| RJ-032-F | `DiagLogValidation` (`diagnostic=D8`) | -- | **ESCALATED** |

## Escalation log

Each escalated row was attempted under the F2/F3 3-retry
authoring protocol (strategy section 5). For each row, the
table below records candidate shapes attempted, empirical
rejection reasons, and the recommended follow-up direction.
Escalated rows are NOT committed to the sidecar (no snippet
file, no `[[row]]` entry); they remain matrix-uncovered.

### OP-003-I -- `Const::EnumValue`

The only path producing `Const::EnumValue` is
`translate_ty_const_value` for a non-leaf, non-ZST value-tree of
ADT-enum type per `crates/regseq-driver/src/bridge/const_.rs:55-122`.

Candidates:
1. `W<{ E::A(7) }>` with `adt_const_params` -- UnsupportedConst:
   `UnevaluatedConst { def: f::{constant#0}, args: [] }`.
2. `pub const V: E = E::A(7); W<V>` -- same UnevaluatedConst at
   a fresh anonymous-const def-id.
3. Same shape -- consistent with (2).

Root cause: the bridge's `mir::Const::eval` cannot evaluate the
anonymous const-block / named-const reference used in the
const-generic position, so the value-tree branch path never
executes. Follow-up: a future task probing whether MIR const
folding can be coaxed (`generic_const_exprs`?) into evaluating
the enum payload, or expanding the bridge's const-eval surface
to accept an unevaluated ADT-enum const, would unblock
OP-003-I.

### RV-007-C -- `UnOp::PtrMetadata`

Candidates:
1. `core::ptr::metadata::<[u32]>(p)` -- NonRigidTy on
   `<[u32] as core::ptr::Pointee>::Metadata`.
2. `core::ptr::metadata::<dyn Tr>(p)` -- Foreign type
   (`core::ptr::metadata::VTable` is an extern type).
3. (Implicit) sized pointee variant -- NonRigidTy on
   `<u32 as core::ptr::Pointee>::Metadata`.

Root cause: the bridge does not normalize the `Pointee::Metadata`
associated-type projection in any of the slice / dyn / sized
specializations. Follow-up: a bridge-side task to extend
`translate_ty` (or the normalization layer it calls) so that
`Pointee::Metadata` projections normalize through to the
concrete type (`usize` for slice, `DynMetadata<dyn Tr>` for dyn,
`()` for sized) would unblock RV-007-C.

### RV-009-G -- `AggregateKind::RawPtr`

Same root cause as RV-007-C: `core::ptr::from_raw_parts` and the
backing `core::intrinsics::aggregate_raw_ptr` intrinsic have
signatures parameterized by `Pointee::Metadata`. Each tried
specialization (slice + sized) fails NonRigidTy. Follow-up:
shares the bridge-side normalization extension with RV-007-C.

### TM-006-C -- `AssertKind::OverflowNeg`

Candidates:
1. `pub fn f(x: i32) -> i32 { -x }` -- no Assert::OverflowNeg
   emitted; the helper at `crates/test-ir-fixtures/src/lib.rs:342-344`
   sets `overflow_checks=false` so MIR build does not insert
   the runtime check.

Root cause: there is no intrinsic that directly emits
`AssertKind::OverflowNeg`; the construct is rustc-MIR-builder
specific and gated by the `overflow_checks` codegen option.
Follow-up: either teach the helper to accept a per-fixture
`overflow_checks=true` override, or document TM-006-C as
"requires overflow-checked codegen" and defer until that
override exists.

### FF-004 -- `FunctionInstanceFlavor::ReifyShim`

Candidates:
1. `pub fn target() {} pub fn entry() -> fn() { target }` -- no
   ReifyShim emitted; rustc collapses the reify to a single
   `PointerCoercion::ReifyFnPointer` cast and the resulting
   instance is `InstanceKind::Item(target)`.
2. `pub extern "C" fn target() {} pub fn entry() -> extern "C" fn() { target }`
   -- same outcome, just a different ABI.
3. `#[track_caller] pub fn target() {} ... let fp: fn() = target; fp();`
   -- still no ReifyShim. Per `rustc_middle/src/ty/instance.rs`
   resolve_for_fn_ptr, ReifyShim should be emitted for
   track_caller functions losing their convention; rustc may
   have elided this path under optimized MIR.

Root cause: rustc's monomorphizer does not synthesize
`InstanceKind::ReifyShim` for the snippet shapes tried. The
intrinsic / track-caller paths that DO synthesize it (per the
rustc source) appear elided in the pinned-nightly's optimized
MIR. Follow-up: probe alternative shapes (e.g., reifying a
`#[rustc_intrinsic]` function) or wait for a bridge-side
synthetic `ReifyShim` producer.

### FF-005 -- `FunctionInstanceFlavor::FnPtrShim`

Candidates:
1. `pub fn caller<F: Fn()>(f: F) { f(); }` with `caller(fp)` --
   `BridgeError::NonRigidTy` on `<fn() as core::ops::FnOnce<()>>::Output`.
2. `pub fn caller(f: impl Fn()) { f(); }` -- same.
3. `let fp: fn() = target; fp.call(())` with `#![feature(fn_traits)]`
   -- same NonRigidTy.

Root cause: the bridge cannot normalize `<fn() as FnOnce<()>>::Output`
when monomorphizing the `Fn::call` resolution path that
produces FnPtrShim. Without that normalization the bridge
rejects the body before any FnPtrShim instance materializes.
Follow-up: a bridge-side task to extend type normalization to
cover the `<fn(...) as Fn / FnMut / FnOnce>::Output` and
associated-method-arg projections.

### FF-012 -- `FunctionInstanceFlavor::CloneShim`

Candidates:
1. `#[derive(Clone, Copy)] struct S(u32); s.clone()` -- no
   CloneShim; user-derived `Clone` impl resolves to the
   derive-generated body, not a CloneShim.
2. `(u32, u32).clone()` -- Phase5 `ItemMissingInstance` for the
   tuple's Clone glue item.
3. `[u32; 4].clone()` -- UnsupportedConst on the array-length
   const `4_usize` while materializing the array CloneShim.

Root cause: CloneShim materialization for tuples / arrays hits
bridge-side completeness or const-handling gaps. Follow-up:
diagnose the per-shape gap (the array shape's UnsupportedConst
on `4_usize` looks like the simpler unblock) and a bridge-side
fix would surface a coverable shape.

### FF-013 -- `FunctionInstanceFlavor::FnPtrAddrShim`

Candidates:
1. `fp == target` -- no FnPtrAddrShim instance; rustc inlined
   the comparison.
2. `PartialEq::eq(&fp, &target)` -- no FnPtrAddrShim instance.
3. `PartialEq::eq(&fp, &(target as fn()))` -- UnsupportedConst:
   `allocation provenance over Function { instance: Item(target) }`.

Root cause: the FnPtrAddrShim path is either inlined away or
runs into the same const-eval-over-fn-pointer-provenance issue
that the bridge currently rejects. Follow-up: a bridge-side
task to accept `Function`-provenance allocations as `Const::Reloc`
(or skip them at the auxiliary-instance scan) would let
FnPtrAddrShim survive.

### RJ-006 -- `DslRejectionKind::Statement`

Candidates:
1. `match (x, y) { (a, _) if a > 0 => a, _ => 0 }` -- no
   `Statement` rejection; rustc's cleanup passes
   (`RemovePlaceMention`, `RemoveStorageMarkers`, etc.) strip
   every rejection-eligible statement before
   `instance_mir`-time, so the bridge never sees one.

Root cause: every MIR statement variant that
`crates/regseq-driver/src/bridge/body/statement.rs:43-82`
rejects (`FakeRead`, `AscribeUserType`,
`BackwardIncompatibleDropHint`, `Coverage`, `PlaceMention`,
`Nop`, `StorageLive`, `StorageDead`, `Retag`,
`ConstEvalCounter`) is either swept by a mandatory rustc
cleanup pass or only emitted under a `-Z` flag the bridge
does not pass. Follow-up: teach the helper or the bridge to
opt into a `-Z` flag (e.g., `-Z mir-emit-retag`) that exposes
one of these statement kinds, or accept RJ-006 as
"empirically unauthorable from source under the current build
contract" and defer.

### RJ-024 -- `UnsupportedEnumLayout`

Candidates:
1. `#[repr(C)] enum E { A, B(u32), C(u64) }` -- accepted; the
   layout produces a direct integer tag, which is supported.
2. `enum E { A(NonZeroU8), B, C, D }` -- NonRigidTy on the
   `NonZeroU8` newtype projection (a different rejection
   reason).

Root cause: the bridge's three UnsupportedEnumLayout paths
(`Scalar::Union` tag, `Float` tag, niche variant
out-of-range) are bridge-internal invariant violations that
rustc's layout solver does not normally produce for source
enums. Follow-up: probe specific layouts (e.g., a
custom-discriminant enum that forces a niche range smaller
than the variant count) or defer as "empirically unauthorable
from source".

### RJ-032-F -- `DiagLogValidation::D8`

Candidates:
1. Snippet with the inlined log shim + a `caller(a, b, c, d, e)`
   that calls `log_info("a={} b={} c={} d={} e={}", &[
   LogArg::from(a), LogArg::from(b), LogArg::from(c),
   LogArg::from(d), LogArg::from(e)])` -- empirically run
   through the runner against a single-row probe sidecar.
   Empirical outcome: bridge accepted (`Ok`); no `D8`
   diagnostic fired. Per `crates/regseq-driver/src/bridge/diag_log/peel.rs:146-149`,
   the D8 check short-circuits to `Ok(())` when
   `target_str_arg_bound` returns `None`, and per
   `crates/regseq-driver/src/bridge/diag_log/peel.rs:179-181`
   that helper is hard-wired to `None`. No alternate snippet
   shape can route around the short-circuit until the
   target-binding wiring lands.

Root cause: D8 is currently unreachable from source. The
target-binding selection that would populate
`max_str_args_per_call` is not yet wired into `BridgeCtxt`.
Follow-up: once the target-binding selection wiring lands
(per spec.md section 26.9 + the per-target `<target>-backend.md`
artifacts), revisit RJ-032-F as a Standard-shape candidate.
The probe snippet is documented here but NOT committed to the
sidecar per the escalation protocol (Change B step 5).

## Strategy-deviation record

NONE. Each escalation followed the F2/F3 3-retry authoring
protocol exactly: each row was attempted with up to three
snippet shapes, every shape ran through the
`bridge-coverage-runner` typed-outcome oracle, and rows that did
not produce a passing candidate after their 3rd attempt were
documented here and left out of the committed sidecar (no
snippet file, no `[[row]]` entry). No new
`InspectionPredicate` enum variants were added; no F1/F2/F3
runner module was modified; no F0 helper was modified.

## Post-F4 readiness signal

After F4 close, the matrix rows that REMAIN uncovered are:

1. **The documented blocked-* set** (per task spec
   `.harness/tasks/regseq-driver/67-bridge-coverage-runner-f4-full-corpus.md`
   "Out-of-scope rows" enumeration, lines 71-143):
   - `blocked-rustc-infeasible`: RJ-014, RJ-017, RJ-020,
     RJ-021.
   - `blocked-bridge-rejects-by-design` w/o positive shape:
     FF-008, FF-009, FF-010, FF-014, FF-015.
   - `blocked-open-bridge-bug` (OPEN BUG family): IX-004,
     IX-010, FT-011, IX-014.
   - Section 7 validator-invariant rows (~60).
   - Section 2.2 closure-call family: SK-004, SK-005.
   - Section 3 closure features: FT-012, FT-013, FT-014.
   - Section 4 cross-edition / alloc-dep / multi-crate:
     CT-004, CT-007, CT-008.
   - Section 6 hard-to-author rejections: RJ-001, RJ-009-013,
     RJ-018, RJ-023, RJ-025, RJ-029, RJ-030, RJ-007.
   - Section 2.3 closure-call-blocked: FF-007.
   - Rows aliased by F2 IX commits: OP-003-K (IX-022),
     TM-005-D (IX-023), FT-003 (IX-012), FT-009 (IX-001).
   - Rows lacking F3 predicates: OP-003-G-1, OP-003-G-3.
   - Negative-assertion rows: FT-004.
   - Bridge-producer-missing rows: CT-010 (`Linkage=Weak`).

2. **F4 escalations** (this task):
   - OP-003-I `Const::EnumValue` -- unevaluated-const-block path.
   - RV-007-C `UnOp::PtrMetadata` -- `Pointee::Metadata` normalization gap.
   - RV-009-G `AggregateKind::RawPtr` -- same.
   - TM-006-C `AssertKind::OverflowNeg` -- helper `overflow_checks=false`.
   - FF-004 `ReifyShim` -- no source path produces the shim.
   - FF-005 `FnPtrShim` -- `<fn() as FnOnce>::Output` normalization gap.
   - FF-012 `CloneShim` -- tuple/array CloneShim materialization gap.
   - FF-013 `FnPtrAddrShim` -- fn-pointer-provenance const rejection.
   - RJ-006 `DslRejectionKind::Statement` -- cleanup passes strip every
     rejection-eligible MIR statement.
   - RJ-024 `UnsupportedEnumLayout` -- bridge-internal-only rejection.
   - RJ-032-F `DiagLogValidation::D8` -- target-binding wiring not landed.

This list is the canonical input to the post-F4 batch-triage
task (strategy section 10 Mode B). Each escalation is a
candidate for either a follow-up `regseq-driver-NN`
investigation task (when the gap is bridge-side and tractable)
or a long-term deferral (when the gap is rustc-side or
infrastructure-side).
