# Needs-validation tail triage -- 2026-05-25

Deliverable of `regseq-driver-80` (Type-B analytical; feature-complete gate F-b
feeder). The feature-complete gate (G1 / driver-79) produced verdict `not-yet`
with **47 Sections-1-6 `needs-validation` rows** (+ 3 Section-7 residuals handled
elsewhere). This document triages all 47 into an actionable partition
(author-now / needs-bridge-fix / reclassify-adjudicated / aggregate-modeling),
each backed by a **HEAD probe** through the real bridge via
`try_bridge_fixture_path` (`crates/test-ir-fixtures/src/lib.rs:223`).

**PROPOSAL artifact only** (Mode B). No committed source, no matrix write, no
`coverage_drift.py`/ledger change, no `task.py add-task`. Probes are throwaway
(deleted before commit), per the R-series feasibility-probe convention and the
playground rule. Mirrors the structure of the F68 triage
(`.harness/audits/2026-05-21-post-f4-triage.md`).

## 0. Method + set parity

The 47 rows are the live ledger's `truth == "needs-validation"` set, re-derived
at HEAD via `uv run python scripts/python/coverage_drift.py` (NOT hard-coded):

```
Section 1 (22): OP-003-C, OP-003-F, OP-003-G-1, OP-003-G-3, OP-003-K, RV-003,
                RV-005, RV-006, RV-006-A, RV-006-I, RV-007, RV-009, ST-003,
                TM-005-A, TM-005-C, TM-005-D, TM-006, TM-006-B, TM-006-F,
                TM-006-G, TM-006-H, TY-005
Section 2 (3):  SK-001, SK-006, SK-011
Section 3 (6):  FT-003, FT-004, FT-009, FT-017, FT-019, FT-021
Section 4 (1):  CT-004
Section 5 (2):  IX-016, IX-020
Section 6 (13): RJ-001, RJ-010, RJ-011, RJ-012, RJ-013, RJ-018, RJ-023, RJ-025,
                RJ-028, RJ-029, RJ-030, RJ-031, RJ-032
```

Per row: (1) identify the construct + intended witness from the matrix row; (2)
probe HEAD with a minimal `#![no_std]` witness through `try_bridge_fixture_path`
(reconstructing playground shapes as throwaway probes; never depending on
`tests/fixtures/playground*`); (3) classify into exactly one bucket; (4) note
`BuildOverrides` needs. Probe outcomes below are verbatim
`BridgeOutcome`/`BridgeError` `Display`/`Debug` text.

**Buckets:** **A** author-now (a minimal witness bridges `Ok`+`validate()` today,
or -- for `rejected`-expectation rows -- produces the spec-expected rejection);
**B** needs-bridge-fix (a realizable witness ICEs, is wrongly rejected, or
bridges to an invalid model); **C** reclassify-adjudicated (no realizable
`#![no_std]` witness / rejects-by-design / unimplemented-phase -> leaves
needs-validation as `blocked-*`/out-of-scope); **D** aggregate/modeling (coverage
defined by sub-axes -> a matrix/tool modeling note, not a fixture).

## 1. Per-row triage table

### Section 1 -- IR vocabulary (22 rows)

| id | construct | expect | probe outcome (verbatim) | bucket | witness / root-cause / target-status | follow-up |
| -- | --------- | ------ | ------------------------ | ------ | ------------------------------------ | --------- |
| OP-003-C | `Const::Int(IntKind,u128)` | positive | `Ok`; consts=`[Int(I32, 7)]` | A | witness `pub fn f()->i32{7}`; predicate: const pool has `Const::Int`. | F-b1 |
| OP-003-F | `Const::ZST` | positive | `Ok`; consts=`[ZST]` (via fn-ptr coercion / unit const) | A | witness `fn g(){} pub fn f(){let _p: fn()=g;}` OR `const U:()=(); use U`; predicate: `Const::ZST` present. (A bare unit-struct return `Z` interned NO const -- the ZST const arises from a ZST `FnDef` coercion or a unit-typed const item.) | F-b1 |
| OP-003-G-1 | `Byte::Uninit` | positive | `Ok`; consts=`[Bytes([Value(1),Uninit,Uninit,Uninit,Value(2),...])]` | A | witness `#[repr(C)] struct S{a:u8,b:u32} const C:S=S{a:1,b:2}; use C` (padding bytes -> `Byte::Uninit`); predicate: a `Const::Bytes` contains `Byte::Uninit`. | F-b1 |
| OP-003-G-3 | `Byte::Reloc(InstanceRef)` | positive | `Ok`; consts=`[Bytes([Reloc(InstanceRef{...}),...,Uint(U32,9)])]` | A | witness `#[repr(C)] struct S{p:*const u32,x:u8} static Q:u32=9; const C:S=S{p:&Q as *const u32,x:7}; use C` (pointer field -> reloc byte interleaved with value bytes); predicate: a `Const::Bytes` contains `Byte::Reloc`. | F-b1 |
| OP-003-K | `Const::Reloc(InstanceRef)` | positive | `Ok`; consts=`[Reloc(InstanceRef{...})]` | A | witness `fn g(){} pub const P: fn()=g; pub fn f(){(P)()}` (scalar fn-ptr const -> top-level `Const::Reloc`); predicate: `Const::Reloc` present. (NB: `let p: fn()=g` interns `Const::ZST`, not Reloc -- the *const item* path produces the Reloc.) | F-b1 |
| RV-003 | `Rvalue::Ref(Provenance,Place)` | positive | `Ok` (items=1) | A | witness `pub fn f()->u32{let a=5u32; let r=&a; *r}`; predicate: a body Rvalue is `Rvalue::Ref`. | F-b2 |
| RV-005 | `Rvalue::Cast` | positive | rep `x as u8` -> `Ok`; 13 sub-axes `validated-runner` + RV-005-F `validated-legacy` | D | aggregate (per-`CastKind`). Coverage satisfied (>=1 covered: 13 runner + 1 legacy). Modeling: roll up aggregate from sub-axes. | D-rollup |
| RV-006 | `Rvalue::BinaryOp` | positive | rep `a&b` -> `Ok`; 24 sub-axes `validated-runner` (RV-006-A/-I are themselves needs-validation rows, bucketed A above) | D | aggregate (per-`BinOp`). Coverage satisfied (24 runner sub-axes covered). | D-rollup |
| RV-006-A | `BinOp::Add` | positive | `Ok` (overflow off -> plain `Add`) | A | witness `pub fn f(a:u32,b:u32)->u32{a+b}`; predicate: `BinaryOp(Add,..)`. | F-b2 |
| RV-006-I | `BinOp::AddWithOverflow` | positive | `Ok` **with `BuildOverrides{overflow_checks:Some(true)}`** | A | witness `pub fn f(a:i32,b:i32)->i32{a+b}` + overflow override (driver-69 `z_flags`/`overflow_checks` surface, EXISTS). predicate: `BinaryOp(AddWithOverflow,..)`. Ties to TM-006-B. | F-b2 |
| RV-007 | `Rvalue::UnaryOp` | positive | rep `-x` -> `Ok`; sub-axes RV-007-A/B/C `validated-runner` | D | aggregate (per-`UnOp`). Sub-axes covered (incl. RV-007-C `UnOp::PtrMetadata`, the A1 alias case, now validated-runner). | D-rollup |
| RV-009 | `Rvalue::Aggregate` | positive | rep `(1,2)` -> `Ok`; RV-009-A/B/E/G `validated-runner` + RV-009-C/D/F `validated-legacy`/universal | D | aggregate (per-`AggregateKind`). Coverage satisfied (4 runner + 3 legacy/universal). | D-rollup |
| ST-003 | `StatementKind::Intrinsic` | positive | rep `copy_nonoverlapping` -> `Ok`; ST-003-A/B `validated-runner` | D | aggregate (per non-diverging intrinsic). Sub-axes covered. | D-rollup |
| TM-005-A | `CallCallee::Direct(InstanceId)` | positive | `Ok` (items=2) | A | witness `fn g(){} pub fn f(){g()}`; predicate: a `Call` terminator with `CallCallee::Direct`. ("every direct fn call" universal phrase the evidence-classifier didn't match -> commit a row; alt: tool universal-detection refinement.) | F-b2 |
| TM-005-C | `Call.target==Some(bb)` | positive | `Ok` (items=2) | A | same witness as TM-005-A (a non-diverging direct call has `target==Some`). | F-b2 |
| TM-005-D | `Call.target==None` (diverging) | positive | `Ok` (items=2) | A | witness `fn diverge()->!{loop{}} pub fn f(){diverge()}`; predicate: a `Call` terminator with `target==None`. | F-b2 |
| TM-006 | `TerminatorKind::Assert` | positive | rep overflow assert -> `Ok`; TM-006-A/C/D/E `validated-runner` | D | aggregate (per-`AssertKind`). Coverage satisfied (4 runner sub-axes). TM-006-B (A) + TM-006-F/G/H (C) are themselves needs-validation rows triaged above. | D-rollup |
| TM-006-B | `AssertKind::Overflow(BinOp,..)` | positive | `Ok` **with `overflow_checks:Some(true)`** | A | witness `pub fn f(a:i32,b:i32)->i32{a+b}` + overflow override; predicate: `Assert{ msg: AssertKind::Overflow ..}`. | F-b2 |
| TM-006-F | `AssertKind::MisalignedPointerDereference` | positive | `*p` deref bridges `Ok` (items=1) **without the assert** | C | these three assert kinds are inserted by rustc's `CheckAlignment`/`CheckNull`/`CheckEnums` UB-check passes (rustc `compiler/rustc_mir_transform/src/lib.rs:709-711`); the regseq production MIR pass whitelist disables `CheckAlignment` (`crates/regseq-driver/src/callbacks.rs:32`, value `false`), so the inserting pass never runs and this assert is unreachable in any regseq-bridged MIR. target: `blocked-rustc-infeasible` (build-contract; per RJ-024-style usage) / gate `out-of-scope`. | C-reclass |
| TM-006-G | `AssertKind::NullPointerDereference` | positive | `*p` deref bridges `Ok` (items=1) **without the assert** | C | `CheckNull` disabled (`callbacks.rs:33`, `false`). Same disposition as TM-006-F. | C-reclass |
| TM-006-H | `AssertKind::InvalidEnumConstruction(Operand)` | positive | `transmute::<u8,E>` bridges `Ok` (items=2) **without the assert** | C | `CheckEnums` disabled (`callbacks.rs:34`, `false`). Same disposition as TM-006-F. | C-reclass |
| TY-005 | `Ty::Float(FloatKind)` | positive | `f32` -> `Ok`; `f64` -> `Ok` | A | witness `pub fn f(x:f32)->f32{x}` (+`f64`); predicate: `Ty::Float` present. (`f16`/`f128` need feature gates; `f32`/`f64` cover the row.) | F-b2 |

### Section 2 -- synthetic items (3 rows)

| id | construct | expect | probe outcome (verbatim) | bucket | witness / root-cause | follow-up |
| -- | --------- | ------ | ------------------------ | ------ | -------------------- | --------- |
| SK-001 | `SyntheticItemKind::None` (source-derived) | positive | `Ok` (items=1) on any source item | A | witness any source fn (`no_op`); predicate: an Item with `SyntheticItemKind::None`. ("most fixtures" universal phrase demoted by the classifier -> commit a row; alt: tool universal-detection refinement.) | F-b3 |
| SK-006 | `SyntheticItemKind::ClosureDropGlue{closure}` | positive | `Ok` (items=9) | A | witness `struct D; impl Drop for D{..} fn f(){let d=D; let c=move||{let _=&d;}; core::mem::drop(c);}` (closure captures a Drop type by move, then dropped); predicate: a synth Item `ClosureDropGlue`. | F-b3 |
| SK-011 | `SyntheticItemKind::VTableDropShim{principal,self_ty}` | positive | `Ok` (items=12) | A | witness `trait T{fn m(&self);} struct S; impl Drop for S{..} impl T for S{..} fn take(x:&dyn T){x.m()} fn g(){let s=S; take(&s);}` (vtable over a Drop concrete type); predicate: a synth Item `VTableDropShim`. | F-b3 |

### Section 3 -- features (6 rows)

| id | construct | expect | probe outcome (verbatim) | bucket | witness / root-cause | follow-up |
| -- | --------- | ------ | ------------------------ | ------ | -------------------- | --------- |
| FT-003 | Nested Drop fields (field-drop ordering) | positive | **`Rejected: phase 5 body pass failed: joint validation body pre-check failed for instance 3: [BRpo]`** | **B** | **realizable witness wrongly rejected.** `struct Inner; impl Drop for Inner{} struct Outer{a:Inner} impl Drop for Outer{} fn f(){let _o=Outer{a:Inner};}` -> Phase5 reject; a 2-Drop-field shape `Outer{a:Inner,b:Inner}` (no outer impl) ALSO rejects (`instance 2: [BRpo]`). single-Drop and single-Drop-field shapes bridge `Ok`. Root cause: multi-drop-site drop-glue bodies emit basic blocks out of DFS reverse-postorder, tripping the B-RPO body pre-check (`crates/regseq-model/src/visit.rs:74-78`; surfaced at `crates/regseq-driver/src/bridge/body/materialise.rs:414`). | R-series (FT-003-fix) |
| FT-004 | `ManuallyDrop<T>` over a Drop type | positive (NEGATIVE drop-not-scheduled) | `Ok` (items=6) | A | witness `use core::mem::ManuallyDrop; struct D; impl Drop for D{} fn f(){let _m=ManuallyDrop::new(D);}`; predicate (negative): no drop scheduled for the `ManuallyDrop`'d value. | F-b3 |
| FT-009 | Generic free fn called with concrete args | positive | `Ok` (items=2) | A | witness `fn id<T>(x:T)->T{x} pub fn f()->u32{id(7u32)}`; predicate: a monomorphized `FnDef` instance for `id::<u32>`. | F-b3 |
| FT-017 | `const fn` evaluated in const context | positive | `Ok` (items=3) | A | witness `const fn k()->u32{5} pub const C:u32=k(); pub fn f()->u32{C}`. | F-b3 |
| FT-019 | Associated const on an `impl` block | positive | `Ok` (items=3) | A | witness `struct S; impl S{const A:u32=7;} pub fn f()->u32{S::A}`. | F-b3 |
| FT-021 | Diag-log format-template parse rejection (-> RJ-031) | rejected | d3 (`{:?}`) -> `Rejected: diagnostic-logging format error (DebugFormatterRejected) in template ...`; RJ-031-A..F `validated-runner` | D | aggregate-alias of the Section-6 RJ-031 family: the format-parse rejection axis is already covered by the RJ-031 sub-axes (validated-runner). Modeling: FT-021 is the Section-3 mirror of RJ-031; no separate Section-3 fixture needed. | D-rollup |

### Section 4 -- cross-cutting (1 row)

| id | construct | expect | probe outcome (verbatim) | bucket | witness / root-cause / target-status | follow-up |
| -- | --------- | ------ | ------------------------ | ------ | ------------------------------------ | --------- |
| CT-004 | Edition 2024 | positive | a 2021 witness bridges `Ok` (items=1); the helper hard-wires `--edition 2021` | C | the helper has NO edition override (`crates/test-ir-fixtures/src/lib.rs:379-380` pins `--edition 2021`), and edition 2024 introduces **no new RegseqModel IR vocabulary** (it is a compile front-end axis; edition-specific drop-scheduling differences are covered by existing drop rows). target: gate `out-of-scope` (front-end axis, no IR construct). ALTERNATIVE: a one-off helper edition-override (infra, like driver-69's `BuildOverrides`) if an explicit 2024 end-to-end smoke is wanted -- architect decision. | C-reclass (or infra smoke) |

### Section 5 -- interactions (2 rows)

| id | construct | expect | probe outcome (verbatim) | bucket | witness / root-cause | follow-up |
| -- | --------- | ------ | ------------------------ | ------ | -------------------- | --------- |
| IX-016 | Const eval x Const item used in body | positive | `Ok` (items=2) | A | witness `const C:u32=7; pub fn f()->u32{C}`; predicate: a `Const::Int` loaded in a body. | F-b3 |
| IX-020 | Drop impl + assoc consts + inherent methods on same struct | positive | `Ok` (items=7) | A | witness `struct S(u32); impl Drop for S{} impl S{const A:u32=7; fn m(&self)->u32{self.0}} pub fn f()->u32{let s=S(1); s.m()+S::A}` (single-Drop -> NOT the FT-003 multi-drop BRpo path). | F-b3 |

### Section 6 -- rejection variants (13 rows)

| id | construct | expect | probe outcome (verbatim) | bucket | witness / root-cause / target-status | follow-up |
| -- | --------- | ------ | ------------------------ | ------ | ------------------------------------ | --------- |
| RJ-001 | `DslRejectionKind::Box` | rejected | `Rejected: DSL rejects Box type: alloc::boxed::Box<u32>` | A | witness `#![no_std] extern crate alloc; pub fn f(_x: alloc::boxed::Box<u32>){}`; predicate: `BridgeError::DslRejection{kind:Box}` (`bridge/ty.rs:457-459`). Expected by-design rejection. | F-b4 |
| RJ-010 | `NonRigidTyKind::Bound` | rejected | a generic fn (`g<T>(x:T)`) monomorphized -> `Ok` (no `Bound` surfaces) | C | `TyKind::Bound` is a binder variable ("the `'a` in `for<'a> fn(&'a ())`" / canonical-query var) per rustc `compiler/rustc_type_ir/src/ty_kind.rs:236` + its doc `:216-235`; it does not appear in the post-`after_analysis` monomorphic MIR the bridge consumes. No `#![no_std]` source surfaces it (parallels RJ-009 Param "no-surface" reframe + RJ-014 `blocked-rustc-infeasible`). bridge producer `bridge/ty.rs:435`. target: `blocked-rustc-infeasible`. | C-reclass |
| RJ-011 | `NonRigidTyKind::Placeholder` | rejected | (same family as RJ-010; not surfaceable) | C | `TyKind::Placeholder` is "used during higher ranked subtyping to instantiate bound variables" per rustc `rustc_type_ir/src/ty_kind.rs:246` + doc `:243-245`; never in MIR. bridge producer `bridge/ty.rs:436`. target: `blocked-rustc-infeasible`. | C-reclass |
| RJ-012 | `NonRigidTyKind::Infer` | rejected | (same family; not surfaceable) | C | `TyKind::Infer` is an inference variable per rustc `rustc_type_ir/src/ty_kind.rs:254`; inference vars are fully resolved before MIR. bridge producer `bridge/ty.rs:437`. target: `blocked-rustc-infeasible`. | C-reclass |
| RJ-013 | `NonRigidTyKind::Alias` | rejected | `<u32 as Tr>::A` in a `PhantomData` arg -> `Rejected: non-rigid TyKind survived normalization: Alias in <u32 as Tr>::A`; body RPIT `let f=mk()` -> `Rejected: ... Alias in impl Fn() -> u32` | **B** | **realizable, but the surfaced shapes are normalizable.** `<u32 as Tr>::A` is fully concrete (`u32: Tr` with `type A=u8`) and a correct bridge should normalize it to `u8`; it instead survives to the `TyKind::Alias` short-circuit (`bridge/ty.rs:438`). Same normalization family as the 2026-05-21 A1 cluster -- the `Pointee::Metadata` sibling RV-007-C is now `validated-runner`, so RJ-013 is the residual member (fully-concrete projections + body-level RPIT opaque-reveal). The `rejected` expectation is a defensive backstop that should be unreachable on valid mono source (cf. the RJ-009/Param reframe). DECISION for the follow-up: normalization real-fix (preferred) vs adjudicate-as-by-design + author (A). | R-series (RJ-013 normalize-or-adjudicate) |
| RJ-018 | `UnsafeBinder` | rejected | `Rejected: unsafe binder type is unsupported: unsafe<'a> &'a u32` (via `#![feature(unsafe_binders)]` + `wrap_binder!`) | A | witness reaches the bridge and produces the by-design rejection (`bridge/ty.rs:429-432`). Producible -> C/`rustc-infeasible` would be UNSOUND. CAVEAT: depends on the incomplete `unsafe_binders` feature; for consistency with feature-gated RJ-017/020/021 (`blocked-rustc-infeasible | feature gated`) the architect MAY prefer to adjudicate rather than commit a feature-gated snippet. Primary call A (empirically producible). | F-b4 (architect: commit vs adjudicate) |
| RJ-023 | `Layout` | rejected | huge-array `size_of::<[u8;1e11]>` -> `Ok`; `#[repr(simd)]` -> `Ok` (neither surfaces `Layout`) | C | `BridgeError::Layout` is emitted only by `.map_err(|_| Layout)` on `tcx.layout_of` (`bridge/synth/vtable.rs:571`, `const_.rs:292,491`, `ty.rs:1057`). rustc's `layout_of` normalizes (`with_post_analysis_normalized`) before layout and returns `LayoutError` for too-generic / unrepresentable types (rustc `compiler/rustc_ty_utils/src/layout.rs:40-47`); a monomorphic type the bridge reaches post-`after_analysis` was already accepted by rustc and has a computable layout. Defensive invariant, directly parallels RJ-024 (`.harness/audits/2026-05-23-b2-deferred-shim-probe.md`). target: `blocked-rustc-infeasible`. | C-reclass |
| RJ-025 | `UnsupportedTy` | rejected | `Rejected: unsupported type: str` (via `#![feature(unsized_fn_params)] fn f(_x: str)`) | A | bare unsized marker in argument position hits `BridgeError::UnsupportedTy` (`bridge/ty.rs:215-220`). Producible -> C would be UNSOUND. CAVEAT: depends on incomplete `unsized_fn_params`; same architect commit-vs-adjudicate decision as RJ-018. Primary call A. | F-b4 (architect: commit vs adjudicate) |
| RJ-028 | `UnsupportedCAbiType` (per `CAbiTypePosition`) | rejected | RJ-028-A `Rejected: ...Input(0): Tuple{..}`; RJ-028-B `...Output: Tuple{..}`; RJ-028-C `...Field: FnPtr(0)` | D | aggregate (3 sub-axes Input/Output/Field), all `validated-runner` (verified via committed runner snippets). Modeling: roll up from sub-axes. | D-rollup |
| RJ-029 | `UnsupportedConst` | rejected | `Rejected: unsupported const: 3_usize` (via `struct W<const N:usize>; f(_w: W<3>)`) | A | const-generic arg rejection at `bridge/ty.rs:614-618` (`translate_generic_args` rejects `GenericArgKind::Const`). predicate: `BridgeError::UnsupportedConst`. Expected by-design rejection. | F-b4 |
| RJ-030 | `GenericOpaqueHandle` | rejected | (no producer; not surfaceable) | C | **`BridgeError::GenericOpaqueHandle` has NO construction site anywhere in the bridge** (grep: only the enum def `bridge/mod.rs:511-512` + the runner's classifier `bridge-coverage-runner/src/cluster.rs:193`); the `#[rseq::opaque]` attribute is not parsed by the bridge. Unimplemented producer. target: `blocked-unimplemented-phase`. | C-reclass |
| RJ-031 | `DiagLogFormat` (per `DiagLogFormatReason`) | rejected | RJ-031-A `DebugFormatterRejected`; -D `NonSubsetModifier`; -F `UnmatchedClosingBrace` (all `Rejected: diagnostic-logging format error (...)`) | D | aggregate (6 sub-axes), all `validated-runner` (verified via runner snippets, full pipeline). Modeling: roll up from sub-axes. | D-rollup |
| RJ-032 | `DiagLogValidation` (per diagnostic) | rejected | RJ-032-A `D1`; -D `D6`; -E `D7` (all `Rejected: diagnostic-logging validation failed (...)`) | D | aggregate (6 sub-axes); A-E `validated-runner`; **RJ-032-F (D8) already `out-of-scope`** (externally blocked on target-binding wiring per the F68/F1 adjudication, spec.md s.26.9). Modeling: roll up from sub-axes; F stays externally deferred. | D-rollup |

## 2. Roll-up

### 2.1 Per-bucket counts (47 total)

| bucket | count | rows |
| ------ | ----- | ---- |
| **A** author-now | **26** | OP-003-C, OP-003-F, OP-003-G-1, OP-003-G-3, OP-003-K, RV-003, RV-006-A, RV-006-I, TM-005-A, TM-005-C, TM-005-D, TM-006-B, TY-005, SK-001, SK-006, SK-011, FT-004, FT-009, FT-017, FT-019, IX-016, IX-020, RJ-001, RJ-018, RJ-025, RJ-029 |
| **B** needs-bridge-fix | **2** | FT-003 (multi-drop BRpo ordering), RJ-013 (alias normalization residue) |
| **C** reclassify-adjudicated | **9** | TM-006-F, TM-006-G, TM-006-H, CT-004, RJ-010, RJ-011, RJ-012, RJ-023, RJ-030 |
| **D** aggregate/modeling | **10** | RV-005, RV-006, RV-007, RV-009, ST-003, TM-006, FT-021, RJ-028, RJ-031, RJ-032 |

(26 + 2 + 9 + 10 = 47.) No row is `needs-deeper-investigation`; every row classified
within the 3-probe budget.

### 2.2 Proposed follow-up decomposition (PROPOSAL -- architect gates filing)

**(A) Snippet sub-batches** (author committed runner rows + inspection predicates;
F-b feeders). Grouped by family so each is a coherent task-spec session:

- **F-b1 -- const surface (5):** OP-003-C, OP-003-F, OP-003-G-1, OP-003-G-3, OP-003-K.
  All const-pool rows; witnesses + predicates pinned above (verified the precise
  `Const`/`Byte` variant each interns). 2 are scalar (`Int`/`ZST`/`Reloc`), 2 are
  `Bytes`-allocation (`Uninit`/`Reloc`).
- **F-b2 -- rvalue/terminator/float scalars (8):** RV-003, RV-006-A, RV-006-I,
  TM-005-A, TM-005-C, TM-005-D, TM-006-B, TY-005. **RV-006-I + TM-006-B need
  `BuildOverrides{overflow_checks:Some(true)}`** (the driver-69 surface EXISTS);
  the rest use defaults.
- **F-b3 -- drop/closure/synth + generics + const-fn (9):** SK-001, SK-006, SK-011,
  FT-004, FT-009, FT-017, FT-019, IX-016, IX-020. (FT-004 is a NEGATIVE
  drop-not-scheduled predicate; IX-020 is single-Drop, distinct from FT-003.)
- **F-b4 -- rejection snippets (4):** RJ-001, RJ-029 (clean); RJ-018, RJ-025
  (feature-gated -- architect decides commit-feature-gated-snippet vs adjudicate;
  see 2.3 note).

**(B) Real-fix task candidates (2):**

- **FT-003-fix -- multi-drop drop-glue block ordering.** Multi-drop-site drop-glue
  bodies emit basic blocks out of DFS-RPO, tripping the B-RPO pre-check
  (`regseq-model/src/visit.rs:74-78`). Realizable from trivial 2-Drop-field source.
  R-series shape (root cause = body block-ordering in the drop-glue materializer).
  HIGH priority (blocks any nested/multi-Drop fixture). Pending fix, matrix status
  -> `blocked-open-bridge-bug`.
- **RJ-013 -- alias normalization residue (normalize-or-adjudicate).** Fully-concrete
  associated-type projections and body-level RPIT opaques survive to the
  `TyKind::Alias` short-circuit (`bridge/ty.rs:438`). The A1 sibling RV-007-C is
  already fixed/validated-runner; this is the residual. The follow-up DECIDES
  between a normalization extension (preferred -- makes the rejection unreachable,
  cf. RJ-009/Param reframe) and adjudicating the rejection as by-design (then RJ-013
  -> A author). MEDIUM priority.

**(C) Reclassification list (9 -- applied by G1b's gated matrix write or a dedicated
reclass task; NOT here):**

| rows | target matrix status | reason |
| ---- | -------------------- | ------ |
| TM-006-F, TM-006-G, TM-006-H | `blocked-rustc-infeasible` (build-contract) | regseq MIR pass whitelist disables `CheckAlignment`/`CheckNull`/`CheckEnums` (`callbacks.rs:32-34`); these asserts never arise in regseq-bridged MIR |
| CT-004 | `out-of-scope` (front-end axis) | edition 2024 adds no new IR vocabulary; helper pins `--edition 2021`. (Optional infra smoke if a 2024 end-to-end check is wanted.) |
| RJ-010, RJ-011, RJ-012 | `blocked-rustc-infeasible` | `Bound`/`Placeholder`/`Infer` TyKinds are inference-internal, erased before monomorphic MIR (cf. RJ-009/014) |
| RJ-023 | `blocked-rustc-infeasible` | `layout_of` does not fail on rustc-accepted monomorphic types; defensive invariant (cf. RJ-024) |
| RJ-030 | `blocked-unimplemented-phase` | no producer for `GenericOpaqueHandle`; `#[rseq::opaque]` unparsed |

**(D) Aggregate modeling dispositions (10 -- a `coverage_drift.py` roll-up
refinement OR matrix note; NOT new fixtures):** RV-005, RV-006, RV-007, RV-009,
ST-003, TM-006, FT-021, RJ-028, RJ-031, RJ-032. Each is an aggregate whose
coverage is defined by its sub-axes, and **every aggregate has >=1 already-resolved
sub-axis** -- resolved by a committed runner row (`validated-runner`), legacy/
universal evidence (`validated-legacy` / `validated-legacy-universal`), or an
accepted out-of-scope adjudication. Exact per-aggregate mix (verified against the
ledger): RV-007/ST-003/RJ-028/RJ-031 = all `validated-runner`; RV-005 = 13 runner
+ 1 legacy; RV-009 = 4 runner + 3 legacy/universal; RV-006 = 24 runner (its
RV-006-A/-I sub-axes are themselves bucket-A rows here); TM-006 = 4 runner (its
B/F/G/H sub-axes are bucket-A/-C rows here); RJ-032 = 5 runner + RJ-032-F
`out-of-scope`. Disposition: derive an aggregate row's coverage as
`covered iff >=1 (resp. all enumerated) sub-axes resolved`, so these leave
`needs-validation` WITHOUT new fixtures. RJ-032-F (D8) stays externally deferred
(F1, target-binding wiring). FT-021 rolls up to the RJ-031 family.

### 2.3 Revised feature-complete picture

Starting set: **47** needs-validation rows.

- **-10 (D)**: aggregate rows are already covered via already-resolved sub-axes
  (runner / legacy / universal / accepted out-of-scope; per-aggregate mix in 2.2);
  resolved by a roll-up modeling rule (tool/matrix), not fixtures.
- **-9 (C)**: reclassify to `blocked-*`/out-of-scope (rustc-infeasible /
  build-contract-suppressed / producer-missing / front-end-axis); they leave the
  worklist with no implementation.

**True remaining author/fix work: 28 rows** = **26 author-now snippets (A)** +
**2 bridge fixes (B)**. Of the 26 A rows: 2 need the existing `overflow_checks`
override (RV-006-I, TM-006-B); 2 are feature-gated rejections pending an architect
commit-vs-adjudicate call (RJ-018, RJ-025); the remaining 22 bridge `Ok`/produce
the expected rejection on plain `#![no_std]` source today.

So the bridge is **much closer to IR-structurally feature-complete than "47"
suggests**: ~19 rows (10 D already-covered + 9 C out-of-scope) are not real gaps,
2 are genuine bridge bugs (FT-003, RJ-013), and 26 are commit-a-snippet author
work. The gate verdict flips to `pass` once the F-b snippet batches land, the 2
B fixes land (or RJ-013 is adjudicated), and the C reclassifications + D roll-up
are applied -- none of which is blocked on external work except RJ-032-F (already
accepted/deferred). The s.6.5 caveat (enumerated coverage != exhaustiveness proof)
still applies.

## 3. Discipline checks

- **All 47 triaged once, each with a verbatim HEAD probe outcome** (s.1 tables;
  set parity against the live ledger confirmed in s.0). No row force-fit; none
  `needs-deeper-investigation`.
- **Probes throwaway**: all witnesses were ephemeral temp-file probes through
  `try_bridge_fixture_path` (+ the existing committed runner snippets for the D
  aggregates, read-only). Nothing committed; no `tests/fixtures/playground*`
  dependency.
- **No auto-filing / no matrix write / no tool or ledger change**: this document
  PROPOSES the decomposition; the only `tasks.json` mutation is driver-80's own
  status flip. Reclassifications (C) and the D roll-up are explicitly deferred to
  G1b / a dedicated reclass task; A snippets to F-b; B fixes to R-series.
- **Bucket-C soundness**: every C cites concrete evidence (rustc/bridge source or
  a probe) -- TM-006-F/G/H cite the disabled passes (`callbacks.rs:32-34`);
  RJ-010/011/012 cite inference-only TyKinds erased before MIR; RJ-023 cites the
  defensive `layout_of` guards (parallel RJ-024); RJ-030 cites the missing
  producer. **No C row was claimed rustc-infeasible when a probe produced it**
  (RJ-018, RJ-025, RJ-029, RJ-001 all bridged to their rejection -> A, not C).
- **Bucket-B soundness**: both B rows have a reproduced realizable failure
  (FT-003 `[BRpo]` Phase5 reject on a 2-Drop-field shape; RJ-013 `NonRigidTy{Alias}`
  on a fully-concrete projection) with a root-cause anchor.
