# Bridge Coverage Matrix (v2)

Systematic enumeration of every construct, axis, and interaction the
`regseq-driver` bridge admits, alongside its expected behavior and a
status tick mapping each row to a fixture that exercises it (or marking
why no such fixture is realisable).

## How to read this matrix

Each row pairs a `(spec_source, construct_or_axis)` with:

- **expectation_class**:
  - `positive` -- bridge returns `Ok(model)` with `model.validate() == Ok(_)`.
  - `rejected` -- bridge returns `Err(BridgeError::<variant>)` of an
    explicitly-spec'd rejection family (see Section 6 of this file).
  - `validator-failing` -- bridge returns `Ok(model)` but
    `model.validate()` returns `Err(InvariantViolation::<variant>)`.
    Usually a constructed-model test, not a source-level fixture.
- **status** (closed taxonomy):
  - `covered` -- at least one committed fixture exercises this row.
  - `partial` -- a fixture interns the construct OR a closely-related
    sub-shape, but not the full set of sub-axes that warrant a row.
    Notes column names what is partial.
  - `uncovered` -- no committed fixture; a fixture is realisable.
  - `blocked-spec-gap` -- the spec describes a construct with no
    minimal realisable Rust witness; signals a spec ambiguity or
    over-constraint to escalate.
  - `blocked-rustc-infeasible` -- rustc itself cannot produce the
    shape from valid `#![no_std]` Rust (e.g., MIR construct only
    appearing in shims rustc generates, never source-derivable).
  - `blocked-bridge-rejects-by-design` -- the construct exists in
    rustc's surface but the bridge intentionally rejects it via a
    documented `BridgeError` variant. The rejection row in Section
    6 is the load-bearing one; the positive-section row marks the
    negative-status cross-reference.
  - `blocked-generator-gap` -- realisable in Rust but the fixture-
    author hasn't found a minimal shape yet; fixture-author
    follow-up, not a spec issue.
  - `blocked-open-bridge-bug` -- a realisable shape that currently
    panics or rejects the bridge with a `Phase4` / `Phase5` /
    untyped-internal error. Cross-references the open bug. Distinct
    from `uncovered` because the lack of coverage IS the failure
    mode -- a fixture would be filed as a regseq-driver-NN task,
    not used as a passing tick.
  - `blocked-unimplemented-phase` -- realisable in principle, but the
    bridge/backend code path that would produce the construct is
    deliberately stubbed pending a future implementation phase (e.g.,
    a `None`-returning helper awaiting backend wiring). The construct
    short-circuits rather than reaching the producing path, so it is
    unreachable from source today. Distinct from `blocked-open-bridge-bug`
    (no panic/reject -- the path short-circuits) and from
    `blocked-spec-gap` (the spec DOES describe a realisable witness,
    just not until the phase lands). A fixture becomes authorable when
    the phase is implemented.
- **fixture_path**: relative path under
  `crates/test-ir-fixtures/fixtures/` (or other test surface) for
  covered / partial rows. Multiple rows may share one fixture.
- **notes**: rationale, dependencies, or known gotchas.

## Conventions

- One fixture commonly ticks multiple rows; the `fixture_path`
  column lists the ticking fixture per row.
- Negative coverage (`rejected` rows) lives in Section 6; the IR
  vocabulary sections enumerate positive-expectation rows that the
  bridge MUST translate.
- Per the Codex review (round 1) of this matrix, the **interaction
  matrix (Section 5) is the most important section for bug
  discovery** -- single-construct rows in Sections 1-4 do not
  surface the combinatorial bugs (driver-60, driver-61, the open
  prescan args-substitution bug) by design.

## Existing fixture inventory (reference, verified against HEAD)

| Fixture                                | Brief purpose / actual coverage             |
| -------------------------------------- | ------------------------------------------- |
| `closure.rs`                           | Zero-capture closure interned via `drop::<F>`; deliberately AVOIDS `Fn::call`. No `register_tool(rseq)`. |
| `closure_call_sig_sync.rs`             | Zero-capture closure synth via `drop::<F>`; FnSig parallel-table sync (driver-61 B2 guard). No `register_tool(rseq)`. |
| `diag_logging.rs`                      | Inlined `regseq::log` shim; `LogArg` ADT, `log_*` intrinsic call sites. Carries `register_tool(rseq)`. |
| `drop_glue_in_self_consuming_method.rs`| `impl Drop for R`; `R::consume(self)` produces success-path `drop(_1)`; Drop-as-Call collapse (driver-61 B1 firing). No `register_tool(rseq)`. |
| `enum_round_trip.rs`                   | Five `EnumShape` arms (Empty / Single no-scalar / Single scalar / Multiple Direct / Multiple Niche); `AggregateKind::Enum` constructors + `SetDiscriminant` writes. |
| `fn_ptr_translation_sig_sync.rs`       | `fn(u32)->u32` (Rust ABI) + `extern "C" fn(u32)->u32` (C ABI) reified to values; driver-61 B3 guard. |
| `foreign_rendering.rs`                 | Crate-root `extern "C" { fn ...; static ...; }` block; foreign-decl rendering. |
| `instance_completeness.rs`             | `static COUNTER`, `static mut FLAG`, `const LIMIT`, `struct Point`, `enum State`, `union Pair`, `struct Wrap<T>` (generic), trait `Reader` + impl, `&dyn Reader` dispatch, `Cast::Unsize` site. |
| `no_op.rs`                             | `pub fn no_op() {}` -- bare smoke baseline. No source-level constructs beyond the empty fn. |
| `trait_default_method.rs`              | Trait `Greeter` with default body `greet`; `impl Greeter for S {}` without override; driver-60 regression guard. |
| `trait_method_resolution.rs`           | Trait `T` + impl; static-dispatch `s.m()` call-site canonicalization (driver-56). |
| `trait_object.rs`                      | Trait `Reader` + impl; `&dyn Reader` dispatch; vtable synthesis. |

Note: only `diag_logging.rs` declares `#![feature(register_tool)] #![register_tool(rseq)]`. Other fixtures are plain `#![no_std]` Rust.

---

# Section 1: IR Vocabulary (`spec.md` §10, §19, §20, §21, §22, §23)

The IR vocabulary is the producer-side surface RegseqModel admits. Each
row is a `positive` expectation unless otherwise marked.

## 1.1 `Ty` enum -- `crates/regseq-model/src/ty.rs:222` + spec.md §10.0 (19 variants)

| id      | construct                  | expectation | status         | fixture_path                       | notes |
| ------- | -------------------------- | ----------- | -------------- | ---------------------------------- | ----- |
| TY-001  | `Ty::Bool`                 | positive    | uncovered      | --                                 | requires a `bool` local, comparison binop output, or `assert!(...)` body |
| TY-002  | `Ty::Char`                 | positive    | uncovered      | --                                 | no fixture uses `char` |
| TY-003  | `Ty::Int(IntKind)`         | positive    | partial        | `instance_completeness.rs` (`i32` in `Point`) | per-IntKind sub-axes uncovered |
| TY-004  | `Ty::Uint(UintKind)`       | positive    | partial        | many                                | per-UintKind sub-axes uncovered for u8/u16/u64/u128/usize |
| TY-005  | `Ty::Float(FloatKind)`     | positive    | uncovered      | --                                 | no fixture uses `f16`/`f32`/`f64`/`f128` |
| TY-006  | `Ty::Str`                  | positive    | covered        | `diag_logging.rs`                  | `&'static str` in `log_*` calls |
| TY-007  | `Ty::Never`                | positive    | blocked-generator-gap | --                          | `!` requires `loop {}` body or `panic!()` calls |
| TY-008  | `Ty::Array(TyId, u64)`     | positive    | uncovered      | --                                 | no fixture uses `[T; N]` |
| TY-009  | `Ty::Slice(TyId)`          | positive    | covered        | `diag_logging.rs`                  | `LogArg<'_>` slice arg |
| TY-010  | `Ty::Ref(TyId, Provenance)` | positive   | covered        | many                                | `&self` ubiquitous; `&mut self` covered via `Drop::drop(&mut self)` |
| TY-011  | `Ty::Ptr(TyId, Mutability)`| positive    | covered        | `drop_glue_in_self_consuming_method.rs` | `&raw mut _` prelude in Drop-as-Call collapse |
| TY-012  | `Ty::FnDef(ItemId, GenericArgs)` | positive | covered     | many                                | every direct FnDef call site |
| TY-013  | `Ty::FnPtr(FnSigId)`       | positive    | covered        | `fn_ptr_translation_sig_sync.rs`   | both Rust and C ABI variants |
| TY-014  | `Ty::Dynamic { auto_traits, vtable_ty }` | positive | covered | `trait_object.rs`, `instance_completeness.rs` | `dyn Trait` |
| TY-015  | `Ty::Closure(ItemId, GenericArgs)` | positive | covered  | `closure.rs`, `closure_call_sig_sync.rs` | |
| TY-016  | `Ty::Tuple { fields, layout }` | positive | partial      | many (implicit `()` unit returns) | non-unit tuple `(T, U, ...)` uncovered |
| TY-017  | `Ty::Struct { item, args, layout }` | positive | covered | many                                | every struct fixture |
| TY-018  | `Ty::Enum { item, args, layout }` | positive | covered | `enum_round_trip.rs`               | |
| TY-019  | `Ty::Union { item, args, layout }` | positive | covered | `instance_completeness.rs` (`Pair`) | |

### TY-003 sub-axes (`IntKind` -- 6 variants per `ty.rs:99-105`)

| id        | sub-construct | status | notes |
| --------- | ------------- | ------ | ----- |
| TY-003-A  | `IntKind::I8` | uncovered | |
| TY-003-B  | `IntKind::I16` | uncovered | |
| TY-003-C  | `IntKind::I32` | covered | `instance_completeness.rs` (`Point.x: i32`) |
| TY-003-D  | `IntKind::I64` | uncovered | |
| TY-003-E  | `IntKind::I128` | uncovered | |
| TY-003-F  | `IntKind::Isize` | uncovered | |

### TY-004 sub-axes (`UintKind` -- 6 variants per `ty.rs:129-135`)

| id        | sub-construct | status | notes |
| --------- | ------------- | ------ | ----- |
| TY-004-A  | `UintKind::U8` | covered | `trait_object.rs` (`read_byte() -> u8`) |
| TY-004-B  | `UintKind::U16` | uncovered | |
| TY-004-C  | `UintKind::U32` | covered | many |
| TY-004-D  | `UintKind::U64` | covered | `instance_completeness.rs` (`Wrap<u64>`) |
| TY-004-E  | `UintKind::U128` | uncovered | |
| TY-004-F  | `UintKind::Usize` | uncovered | |

### TY-005 sub-axes (`FloatKind` -- 4 variants per `ty.rs:141-156`)

| id        | sub-construct | status | notes |
| --------- | ------------- | ------ | ----- |
| TY-005-A  | `FloatKind::F16` | uncovered | `f16` requires nightly feature |
| TY-005-B  | `FloatKind::F32` | uncovered | |
| TY-005-C  | `FloatKind::F64` | uncovered | |
| TY-005-D  | `FloatKind::F128` | uncovered | `f128` requires nightly feature |

### TY-010 sub-axes (`Provenance` -- 2 variants per `ty.rs:175-177`)

| id        | sub-construct | status | notes |
| --------- | ------------- | ------ | ----- |
| TY-010-A  | `Provenance::SharedBorrow` (`&T`) | covered | many |
| TY-010-B  | `Provenance::UniqueBorrow` (`&mut T`) | covered | `drop_glue_in_self_consuming_method.rs` (`Drop::drop(&mut self)`) |

### TY-011 sub-axes (`Mutability` on `Ty::Ptr`)

| id        | sub-construct | status | notes |
| --------- | ------------- | ------ | ----- |
| TY-011-A  | `Ty::Ptr(_, Mutability::Not)` (`*const T`) | uncovered | |
| TY-011-B  | `Ty::Ptr(_, Mutability::Mut)` (`*mut T`) | covered | `drop_glue_in_self_consuming_method.rs` |

### TY-014 sub-axes (`Ty::Dynamic.auto_traits` -- per `AutoTrait` enum at `ty.rs:199-203`)

| id        | sub-construct | status | notes |
| --------- | ------------- | ------ | ----- |
| TY-014-A  | `dyn Trait` with no auto traits | covered | `trait_object.rs` |
| TY-014-B  | `dyn Trait + Send` (auto-trait: Send) | uncovered | |
| TY-014-C  | `dyn Trait + Sync` (auto-trait: Sync) | uncovered | |
| TY-014-D  | `dyn Trait + Unpin` (auto-trait: Unpin) | uncovered | |
| TY-014-E  | `dyn Trait + Send + Sync` (multi-auto-trait) | uncovered | |

## 1.2 `Place` and `PlaceElem` -- `crates/regseq-model/src/place.rs:30,70` + spec.md §19 (6 projection variants)

| id      | construct                  | expectation | status         | fixture_path | notes |
| ------- | -------------------------- | ----------- | -------------- | ------------ | ----- |
| PL-001  | `Place { local, projection: [] }` (bare local) | positive | covered | every fixture | |
| PL-002  | `PlaceElem::Deref` (MUST be first per §19.3 I-Deref-first) | positive | covered | `trait_object.rs` (via `&self` deref) | |
| PL-003  | `PlaceElem::Field(FieldIdx, TyId)` | positive | covered | `drop_glue_in_self_consuming_method.rs` (`_1.0`), `instance_completeness.rs` (`p.x`) | |
| PL-004  | `PlaceElem::Index(LocalId)` | positive | uncovered | --           | needs an array/slice indexing fixture |
| PL-005  | `PlaceElem::ConstantIndex { offset, min_length, from_end }` | positive | uncovered | -- | usually from pattern matching on slices |
| PL-006  | `PlaceElem::Subslice { from, to, from_end }` | positive | uncovered | -- | slice patterns |
| PL-007  | `PlaceElem::Downcast(VariantIdx)` -- MUST be immediately followed by `Field` per §19.3 I-Downcast-then-Field | positive | covered | `enum_round_trip.rs` (via enum match path) | |

## 1.3 `Operand` -- `crates/regseq-model/src/operand.rs:30` + spec.md §20.1 (3 variants)

| id      | construct                  | expectation | status         | fixture_path | notes |
| ------- | -------------------------- | ----------- | -------------- | ------------ | ----- |
| OP-001  | `Operand::Copy(Place)`     | positive    | covered        | every fixture | |
| OP-002  | `Operand::Move(Place)`     | positive    | covered        | every fixture | |
| OP-003  | `Operand::Constant(ConstId)` | positive | partial | many | per-`Const`-variant sub-axes below |

### OP-003 sub-axes (`Const` enum -- 11 variants per `crates/regseq-model/src/const_value.rs:20`)

| id        | sub-construct | status | notes |
| --------- | ------------- | ------ | ----- |
| OP-003-A  | `Const::Bool(bool)` | uncovered | -- | requires `bool` literal in body |
| OP-003-B  | `Const::Char(u32)` | uncovered | -- | requires `char` literal |
| OP-003-C  | `Const::Int(IntKind, u128)` | covered | playground (`i32` literals) |
| OP-003-D  | `Const::Uint(UintKind, u128)` | covered | many (e.g., `42_u32`) |
| OP-003-E  | `Const::Float(FloatKind, u128)` | uncovered | depends on TY-005 |
| OP-003-F  | `Const::ZST` | covered | playground (Reg unit-construction) -- needs verification |
| OP-003-G  | `Const::Bytes(Vec<Byte>)` | partial | `diag_logging.rs` (string literal -- ConstMemoryStatic-backed) |
| OP-003-H  | `Const::Aggregate(Vec<ConstId>)` | uncovered | array/tuple literal const |
| OP-003-I  | `Const::EnumValue { variant_idx, payload }` | blocked-bridge-rejects-by-design | enum-constant value. Reclassified 2026-05-23 (was `uncovered`): the only source route to a type-level enum value-tree is a const-generic argument (`W<V>` where `V: E` is an enum const), which the bridge rejects by design at `translate_generic_args` (`crates/regseq-driver/src/bridge/ty.rs:743-748`, `UnsupportedConst` -- cross-ref Section 6 RJ-029) and at reverse materialisation (`crates/regseq-driver/src/bridge/body/materialise.rs:673-684`, coordinated gap with driver-37). More durably, `Const::EnumValue` is deliberately excluded from type-level admissibility -- only scalar consts (Bool/Char/Int/Uint/Float) may be a `GenericArg::Const` per `crates/regseq-model/src/const_value.rs:54-64`. Enum/non-scalar const-generics are out of scope for `#![no_std]` driver specs. Decision + analysis: `.harness/audits/2026-05-23-c1-enum-const-eval-plan.md` |
| OP-003-J  | `Const::NullPtr(TyId)` | uncovered | -- | needs a `core::ptr::null()` or `0 as *mut T` call site |
| OP-003-K  | `Const::Reloc(InstanceRef)` | uncovered | -- | function-pointer literal (e.g., `let p: fn() = foo;`) |

### OP-003-G sub-axes (`Byte` enum -- 3 variants per `const_value.rs:69`)

| id        | sub-construct | status | notes |
| --------- | ------------- | ------ | ----- |
| OP-003-G-1 | `Byte::Uninit` | uncovered | -- | requires `MaybeUninit` or padding bytes |
| OP-003-G-2 | `Byte::Value(u8)` | covered | `diag_logging.rs` (string-literal bytes) |
| OP-003-G-3 | `Byte::Reloc(InstanceRef)` | uncovered | -- | const block containing a reloc-bearing pointer |

## 1.4 `Rvalue` -- `crates/regseq-model/src/rvalue.rs:19` + spec.md §21.1 (9 variants)

| id      | construct                  | expectation | status         | fixture_path | notes |
| ------- | -------------------------- | ----------- | -------------- | ------------ | ----- |
| RV-001  | `Rvalue::Use(Operand)`     | positive    | covered        | every fixture | ubiquitous in `_dst = _src;` |
| RV-002  | `Rvalue::Repeat(Operand, u64)` | positive | uncovered      | --           | `[expr; N]` repeat literal |
| RV-003  | `Rvalue::Ref(Provenance, Place)` | positive | covered    | trait fixtures | `&place` / `&mut place` |
| RV-004  | `Rvalue::RawPtr(Mutability, Place)` | positive | covered | `drop_glue_in_self_consuming_method.rs` | Drop-as-Call prelude |
| RV-005  | `Rvalue::Cast(CastKind, Operand, TyId)` | positive | partial | -- | per-`CastKind` sub-axes below |
| RV-006  | `Rvalue::BinaryOp(BinOp, Operand, Operand)` | positive | partial | playground (`+`) | per-`BinOp` sub-axes below |
| RV-007  | `Rvalue::UnaryOp(UnOp, Operand)` | positive | uncovered | --          | per-`UnOp` sub-axes below |
| RV-008  | `Rvalue::Discriminant(Place)` | positive | covered    | `enum_round_trip.rs` | |
| RV-009  | `Rvalue::Aggregate(AggregateKind, Box<[Operand]>)` | positive | partial | per-`AggregateKind` sub-axes below | |

### RV-005 sub-axes (`CastKind` -- 11 variants per `rvalue.rs:77`)

| id        | sub-construct | status | notes |
| --------- | ------------- | ------ | ----- |
| RV-005-A  | `CastKind::PointerExposeProvenance` | uncovered | `ptr.expose_provenance()` |
| RV-005-B  | `CastKind::PointerWithExposedProvenance` | uncovered | `ptr::with_exposed_provenance(addr)` |
| RV-005-C  | `CastKind::PointerCoercion(ReifyFnPointer)` | uncovered | `let f: fn(...) = some_fn_def;` |
| RV-005-D  | `CastKind::PointerCoercion(UnsafeFnPointer)` | uncovered | `let f: unsafe fn(...) = some_safe_fn;` |
| RV-005-E  | `CastKind::PointerCoercion(ClosureFnPointer)` | uncovered | zero-capture closure → fn pointer |
| RV-005-F  | `CastKind::PointerCoercion(Unsize)` | covered | `instance_completeness.rs`, `trait_object.rs` (`&Point → &dyn Reader`) |
| RV-005-G  | `CastKind::IntToInt` | uncovered | |
| RV-005-H  | `CastKind::FloatToInt` | uncovered | depends on TY-005 |
| RV-005-I  | `CastKind::FloatToFloat` | uncovered | depends on TY-005 |
| RV-005-J  | `CastKind::IntToFloat` | uncovered | depends on TY-005 |
| RV-005-K  | `CastKind::PtrToPtr` | uncovered | |
| RV-005-L  | `CastKind::FnPtrToPtr` | uncovered | |
| RV-005-M  | `CastKind::Transmute` | uncovered | |
| RV-005-N  | `CastKind::Subtype` | uncovered | rare; lifetime subtyping artefact |

### RV-006 sub-axes (`BinOp` -- 26 variants per `rvalue.rs:117`)

| id        | sub-construct | status | notes |
| --------- | ------------- | ------ | ----- |
| RV-006-A  | `BinOp::Add` | covered | playground |
| RV-006-B  | `BinOp::Sub` | uncovered | |
| RV-006-C  | `BinOp::Mul` | uncovered | |
| RV-006-D  | `BinOp::Div` | uncovered | |
| RV-006-E  | `BinOp::Rem` | uncovered | |
| RV-006-F  | `BinOp::AddUnchecked` | uncovered | |
| RV-006-G  | `BinOp::SubUnchecked` | uncovered | |
| RV-006-H  | `BinOp::MulUnchecked` | uncovered | |
| RV-006-I  | `BinOp::AddWithOverflow` | covered | playground (auto-emitted from `+` on signed) |
| RV-006-J  | `BinOp::SubWithOverflow` | uncovered | |
| RV-006-K  | `BinOp::MulWithOverflow` | uncovered | |
| RV-006-L  | `BinOp::BitXor` | uncovered | |
| RV-006-M  | `BinOp::BitAnd` | uncovered | |
| RV-006-N  | `BinOp::BitOr` | uncovered | |
| RV-006-O  | `BinOp::Shl` | uncovered | |
| RV-006-P  | `BinOp::ShlUnchecked` | uncovered | |
| RV-006-Q  | `BinOp::Shr` | uncovered | |
| RV-006-R  | `BinOp::ShrUnchecked` | uncovered | |
| RV-006-S  | `BinOp::Eq` | uncovered | |
| RV-006-T  | `BinOp::Lt` | uncovered | |
| RV-006-U  | `BinOp::Le` | uncovered | |
| RV-006-V  | `BinOp::Ne` | uncovered | |
| RV-006-W  | `BinOp::Ge` | uncovered | |
| RV-006-X  | `BinOp::Gt` | uncovered | |
| RV-006-Y  | `BinOp::Cmp` | uncovered | three-way comparison; rare in source |
| RV-006-Z  | `BinOp::Offset` | uncovered | `ptr.offset(n)` |

### RV-007 sub-axes (`UnOp` -- 3 variants per `rvalue.rs:174`)

| id        | sub-construct | status | notes |
| --------- | ------------- | ------ | ----- |
| RV-007-A  | `UnOp::Not` | uncovered | `!bool_local` |
| RV-007-B  | `UnOp::Neg` | uncovered | `-int_local` |
| RV-007-C  | `UnOp::PtrMetadata` | uncovered | `core::ptr::metadata(ptr)` (slice/dyn metadata) |

### RV-009 sub-axes (`AggregateKind` -- 7 variants per `rvalue.rs:185`)

| id        | sub-construct | status | notes |
| --------- | ------------- | ------ | ----- |
| RV-009-A  | `AggregateKind::Array(TyId)` | uncovered | `[a, b, c]` literal |
| RV-009-B  | `AggregateKind::Tuple(TyId)` | partial | implicit `()` unit ubiquitous; non-unit `(a, b)` uncovered |
| RV-009-C  | `AggregateKind::Struct(TyId)` | covered | many (e.g., `Point { x, y }`) |
| RV-009-D  | `AggregateKind::Enum(TyId, VariantIdx)` | covered | `enum_round_trip.rs` |
| RV-009-E  | `AggregateKind::Union(TyId, FieldIdx)` | uncovered | union constructor; depends on TY-019 |
| RV-009-F  | `AggregateKind::Closure(TyId)` | covered | `closure.rs`, `closure_call_sig_sync.rs` |
| RV-009-G  | `AggregateKind::RawPtr(TyId, Mutability)` | covered | raw-pointer aggregate; sized + slice/str pointees bridge via the unsized-allowed aggregate pointee translation (`translate_ptr_pointee`); dyn-from-parts stays `Rejected(Foreign)` (DynMetadata hazard, A1-deferred) |

## 1.5 `Statement` -- `crates/regseq-model/src/statement.rs:34` + spec.md §22.1 (3 variants)

| id      | construct                  | expectation | status         | fixture_path | notes |
| ------- | -------------------------- | ----------- | -------------- | ------------ | ----- |
| ST-001  | `StatementKind::Assign(Place, Rvalue)` | positive | covered | every fixture | |
| ST-002  | `StatementKind::SetDiscriminant(Place, VariantIdx)` | positive | covered | `enum_round_trip.rs` | |
| ST-003  | `StatementKind::Intrinsic(NonDivergingIntrinsic)` | positive | partial | -- | per-intrinsic sub-axes below |

### ST-003 sub-axes (`NonDivergingIntrinsic` -- 2 variants per `statement.rs:64`)

| id        | sub-construct | status | notes |
| --------- | ------------- | ------ | ----- |
| ST-003-A  | `NonDivergingIntrinsic::Assume(Operand)` | uncovered | `core::intrinsics::assume(...)` -- rare in source |
| ST-003-B  | `NonDivergingIntrinsic::CopyNonOverlapping(_)` | uncovered | typically from `core::ptr::copy_nonoverlapping` |

## 1.6 `Terminator` -- `crates/regseq-model/src/terminator.rs:64` + spec.md §23.1 (6 variants -- Drop is COLLAPSED at bridge, not a TerminatorKind variant)

| id      | construct                  | expectation | status         | fixture_path | notes |
| ------- | -------------------------- | ----------- | -------------- | ------------ | ----- |
| TM-001  | `TerminatorKind::Goto(BlockId)` | positive | covered | many | from `if` / `match` lowering |
| TM-002  | `TerminatorKind::SwitchInt { discr, targets }` | positive | covered | `enum_round_trip.rs` | |
| TM-003  | `TerminatorKind::Return`   | positive    | covered        | every fixture | |
| TM-004  | `TerminatorKind::Unreachable` | positive | partial       | implicit (panic=abort `unwind unreachable` edges) | not the sole terminator of any block in current fixtures |
| TM-005  | `TerminatorKind::Call { callee, args, destination, target }` | positive | covered | many | per-`CallCallee` and per-`target` sub-axes below |
| TM-006  | `TerminatorKind::Assert { cond, expected, msg, target }` | positive | partial | playground (`AddWithOverflow`) | per-`AssertKind` sub-axes below |

### TM-005 sub-axes (`CallCallee` -- 2 variants per `terminator.rs:113`)

| id        | sub-construct | status | notes |
| --------- | ------------- | ------ | ----- |
| TM-005-A  | `CallCallee::Direct(InstanceId)` | covered | every direct fn call |
| TM-005-B  | `CallCallee::Indirect(Operand)` | covered | `fn_ptr_translation_sig_sync.rs` |

### TM-005 sub-axes (`Call.target` -- divergence dichotomy per spec.md §23)

| id        | sub-construct | status | notes |
| --------- | ------------- | ------ | ----- |
| TM-005-C  | `Call.target == Some(bb)` (non-diverging) | covered | every direct fn call |
| TM-005-D  | `Call.target == None` (diverging, e.g., calls `!` return type) | uncovered | requires a `-> !` fn call |

### TM-006 sub-axes (`AssertKind` -- 8 variants per `terminator.rs:141`)

| id        | sub-construct | status | notes |
| --------- | ------------- | ------ | ----- |
| TM-006-A  | `AssertKind::BoundsCheck { len, index }` | uncovered | array/slice indexing with non-const index |
| TM-006-B  | `AssertKind::Overflow(BinOp, Operand, Operand)` | covered | playground (`AddWithOverflow`-via-`+`) |
| TM-006-C  | `AssertKind::OverflowNeg(Operand)` | uncovered | `-i32::MIN` |
| TM-006-D  | `AssertKind::DivisionByZero(Operand)` | uncovered | `a / b` |
| TM-006-E  | `AssertKind::RemainderByZero(Operand)` | uncovered | `a % b` |
| TM-006-F  | `AssertKind::MisalignedPointerDereference { ... }` | uncovered | runtime alignment check |
| TM-006-G  | `AssertKind::NullPointerDereference` | uncovered | runtime null check |
| TM-006-H  | `AssertKind::InvalidEnumConstruction(Operand)` | uncovered | non-exhaustive enum construction |

---

# Section 2: Item / Instance / Body lifecycle axes

Per Codex review (round 1): the spec defines lifecycle / pass / shape
axes the IR vocabulary alone does not capture. The pass-timing rows
from v1 (PT-001..PT-005) are DROPPED because they are positive-by-
construction from every `run_bridge` call -- not fixture-authorable
work. Phase-N internal-error rows (PE-001..PE-004 in v1) are similarly
DROPPED from the fixture backlog; they are tracked only via the open
bridge bugs they surface.

## 2.1 `ItemKind` -- `crates/regseq-model/src/item.rs:113` (6 variants)

| id      | construct        | expectation | status      | fixture_path | notes |
| ------- | ---------------- | ----------- | ----------- | ------------ | ----- |
| IK-001  | `ItemKind::Function` | positive | covered | every fixture | |
| IK-002  | `ItemKind::Static { mutability: Not }` | positive | covered | `instance_completeness.rs` (`COUNTER`) | |
| IK-003  | `ItemKind::Static { mutability: Mut }` | positive | covered | `instance_completeness.rs` (`FLAG`) | |
| IK-004  | `ItemKind::Const` | positive | covered | `instance_completeness.rs` (`LIMIT`), playground (`Reg::REG_0`) | |
| IK-005  | `ItemKind::Struct` | positive | covered | many | |
| IK-006  | `ItemKind::Enum` | positive | covered | `enum_round_trip.rs`, `instance_completeness.rs` (`State`) | |
| IK-007  | `ItemKind::Union` | positive | covered | `instance_completeness.rs` (`Pair`) | |

## 2.2 `SyntheticItemKind` -- `crates/regseq-model/src/synthetic.rs:46` (10 variants)

| id      | construct        | expectation | status      | fixture_path | notes |
| ------- | ---------------- | ----------- | ----------- | ------------ | ----- |
| SK-001  | `SyntheticItemKind::None` (source-derived) | positive | covered | most fixtures | |
| SK-002  | `SyntheticItemKind::ClosureCaptures { closure }` | positive | covered | `closure.rs`, `closure_call_sig_sync.rs` | |
| SK-003  | `SyntheticItemKind::ClosureCall { closure, call_kind: Call }` | positive | covered | `crates/bridge-coverage-runner/snippets/SK-003.rs` | regseq-driver-71 (the `Fn::call` body path that was the OPEN BUG IX-004 now translates cleanly post-driver-62) |
| SK-004  | `SyntheticItemKind::ClosureCall { closure, call_kind: CallMut }` | positive | covered | `crates/bridge-coverage-runner/snippets/SK-004.rs` | regseq-driver-72 sub-task A: committed via the new `InspectionPredicate::SyntheticItemKindSub` arm (Shape A.2.ii), which discriminates the `ClosureCall { call_kind }` sub-axis the flat `SyntheticItemKind` arm collapses to `"ClosureCall"` |
| SK-005  | `SyntheticItemKind::ClosureCall { closure, call_kind: CallOnce }` | positive | covered | `crates/bridge-coverage-runner/snippets/SK-005.rs` | regseq-driver-72 sub-task A: committed via the new `InspectionPredicate::SyntheticItemKindSub` arm (Shape A.2.ii); the CallOnce sub-axis is now provable independently of the flat `"ClosureCall"` tag |
| SK-006  | `SyntheticItemKind::ClosureDropGlue { closure }` | positive | uncovered | -- | closure with non-trivial captures + drop |
| SK-007  | `SyntheticItemKind::DropGlue { self_ty }` | positive | covered | `drop_glue_in_self_consuming_method.rs` | |
| SK-008  | `SyntheticItemKind::VTableType { principal }` | positive | covered | `trait_object.rs`, `instance_completeness.rs` | |
| SK-009  | `SyntheticItemKind::VTableInstance { principal, self_ty }` | positive | covered | `trait_object.rs` | |
| SK-010  | `SyntheticItemKind::VTableMethodShim { ... }` | positive | covered | `trait_object.rs` | |
| SK-011  | `SyntheticItemKind::VTableDropShim { principal, self_ty }` | positive | uncovered | -- | needs `dyn Trait` over a Drop type |
| SK-012  | `SyntheticItemKind::ConstMemoryStatic { content_hash }` | positive | partial | `diag_logging.rs` (string-literal backing) | |

## 2.3 `FunctionInstanceFlavor` -- `crates/regseq-model/src/instance.rs:192` (15 variants)

| id      | construct        | expectation | status      | fixture_path | notes |
| ------- | ---------------- | ----------- | ----------- | ------------ | ----- |
| FF-001  | `Item`              | positive | covered | every fixture | |
| FF-002  | `Intrinsic`         | positive | covered | `diag_logging.rs` (log intrinsics) | |
| FF-003  | `VTableShim`        | positive | covered | `trait_object.rs` | |
| FF-004  | `ReifyShim`         | positive | covered | `crates/bridge-coverage-runner/snippets/FF-004.rs` | regseq-driver-73: a `#[track_caller]` fn reified into a `static FP: fn() = t` surfaces `GlobalAlloc::Function { ReifyShim }` provenance. The const translator now accepts that provenance (the `GlobalAlloc::Function` arm in `const_.rs` `translate_scalar` / `allocation_bytes`), materializing the `ReifyShim`-flavored Function instance via `lookup_or_create_direct_instance` and emitting a `Const::Reloc`. Because `ReifyShim(t)` shares the reified fn's `DefId`, the shim instance lands on the source fn item alongside its primary `Item` instance, so the Phase-5 Bucket-1 completeness ratchet (`validate_bridge_completeness`) was relaxed to bound only the primary (`Item`-flavored) Function-instance count, permitting auxiliary shim instances. The runtime-reify path (`let fp: fn() = tc_fn` -> flavor `Item`, the missing `resolve_for_fn_ptr` adjustment) remains a separate out-of-scope gap. See `.harness/audits/2026-05-23-b2-deferred-shim-probe.md` (B2) and the R1 plan |
| FF-005  | `FnPtrShim`         | positive | covered | `crates/bridge-coverage-runner/snippets/FF-005.rs` | regseq-driver-77: a bare `fn` coerced to a function pointer and invoked through a generic `Fn`/`FnMut`/`FnOnce` bound monomorphises to an `InstanceKind::FnPtrShim` whose `Fn*::call` signature is `extern "rust-call"`, which the bridge rejected with `UnsupportedAbi { RustCall }` at `bridge/fnsig.rs:107-115` BEFORE the FnPtrShim Instance was inserted. RECLASSIFIED `covered`-by-rewrite: the FnPtrShim flavor is intentionally never materialised; `rewrite_fn_ptr_call_shim` (`bridge/body/terminator.rs`) lowers the fn-pointer `Fn*::call` to a `CallCallee::Indirect` through the (deref'd-per-kind: `*move _2` for `Fn`/`FnMut`, by-value `copy _1` for `FnOnce`) fn-pointer receiver, untupling the RustCall args -- a fn-pointer call IS an indirect call -- mirroring how the closure `Fn::call` family is covered via the `ClosureCall` rewrite rather than a raw Item. Two coupled sites support it: `scan_terminator_for_synthesis` skips pre-interning the FnPtrShim callee Item (`mod.rs`, else it orphans `core::ops::Fn::call`), and `translate_drop_as_call` gained a `needs_drop` guard -> `Goto` for the no-op `drop(_1: fn())` the call-fix unmasks. The runner predicate is the broad `CallCallee = Indirect`; the discriminating signal is reaching `Ok` AT ALL. The per-kind/args proof lives in `crates/regseq-driver/tests/bridge_fnptr_shim_indirect_call.rs`. bare-FnDef-via-`Fn` (`call_it(g)` without `as fn()`) is a SEPARATE `::Output`-over-FnDef gap and stays rejected. See the R5 plan (`.harness/audits/2026-05-24-ff005-fnptrshim-rustcall-plan.md`) |
| FF-006  | `Virtual`           | positive | covered | `trait_object.rs` | |
| FF-007  | `ClosureOnceShim`   | positive | covered | `crates/bridge-coverage-runner/snippets/FF-007.rs` | regseq-driver-72 sub-task B: the Fn-as-FnOnce coercion shape resolves to an `InstanceKind::ClosureOnceShim`, whose underlying `FnOnce::call_once` Item was pre-interned by the Phase-2 callee scan but left instance-less because the closure-call rewrite redirects the Call to the `ClosureCall` synth Instance. The fix lazy-allocates a `ClosureOnceShim`-flavored Instance for that Item via `lookup_or_create_direct_instance` (Path B.2.a), so the Phase-5 bucket-1 `ItemMissingInstance` check passes and the model carries the `ClosureOnceShim` flavor. The Phase-5 bucket-1 invariant is preserved, not weakened |
| FF-008  | `ConstructCoroutineInClosureShim` | rejected | blocked-bridge-rejects-by-design | -- | coroutines rejected -- cross-ref Section 6 |
| FF-009  | `ThreadLocalShim`   | rejected | blocked-bridge-rejects-by-design | -- | TLS rejected in phase 1a |
| FF-010  | `FutureDropPollShim` | rejected | blocked-bridge-rejects-by-design | -- | async rejected |
| FF-011  | `DropGlue`          | positive | covered | `drop_glue_in_self_consuming_method.rs` | |
| FF-012  | `CloneShim`         | positive | covered | `crates/bridge-coverage-runner/snippets/FF-012.rs` | A fn-pointer `.clone()` bridges to an `InstanceKind::Function` with flavor `CloneShim` (body absent per `bridge/body/materialise.rs:246-269`), zero bridge changes. NOTE: the "derived `Clone` impl" guess was wrong — `#[derive(Clone)]` makes a user impl (flavor `Item`), and arrays use a const-generic library impl (rejected at `ty.rs:744`, the retired-C1 const-generic gap). The F68 audit's B1 array-length-const fix was RETIRED as mis-scoped (see `.harness/audits/2026-05-23-b1-cloneshim-retirement.md`) |
| FF-013  | `FnPtrAddrShim`     | positive | covered | `crates/bridge-coverage-runner/snippets/FF-013.rs` | A direct `<fnptr as FnPtr>::addr` call (`#![feature(fn_ptr_trait)]`) bridges to an `InstanceKind::Function` with flavor `FnPtrAddrShim` (body absent per `bridge/body/materialise.rs:246-269`), zero bridge changes. The F68 audit's C2 const-fix proposal was RETIRED as mis-scoped: fn-pointer comparison never surfaces this flavor in the bridge model even with the const-provenance rejection removed (see `.harness/audits/2026-05-23-c2-fnptraddrshim-retirement.md`) |
| FF-014  | `AsyncDropGlueCtorShim` | rejected | blocked-bridge-rejects-by-design | -- | async drop rejected |
| FF-015  | `AsyncDropGlue`     | rejected | blocked-bridge-rejects-by-design | -- | async drop rejected |
| FF-016  | `CloneShim` w/ collected field-clone (tuple `.clone()`) | positive | covered | `crates/bridge-coverage-runner/snippets/FF-016.rs` | regseq-driver-74: `(u32, u32).clone()` resolves to an `InstanceKind::CloneShim` on `Clone::clone` (the FF-012 family), but rustc's mono-collector also collects the field-type library impl `<u32 as Clone>::clone` (`core::clone::impls::{impl#15}::clone`) as a mentioned mono-item. The bridge interns it yet leaves it instance-less: its only call edge is inside the CloneShim body, which the bridge leaves absent and which rustc has optimized to a plain field copy, so no reactive call site instantiates it -> Phase-5 `ItemMissingInstance` at HEAD. The Change-B post-pass (`instantiate_orphaned_collected_functions`) gives every source bucket-1 zero-instance Function item the `Item`-flavored `body: None` instance a real caller would have produced, recovering the rustc resolution stashed at the Phase-2 `MonoItem::Fn` walk (Change A). The runner predicate is coarse (any `CloneShim`-flavored instance, same as FF-012); the discriminating signal is that the row reaches `Ok`+validate AT ALL. The exact field-clone-instance proof (`::clone` Item, `body: None`) lives in `crates/regseq-driver/tests/bridge_orphan_collected_instance.rs` |
| FF-017  | Zero-arg closure call (constant `const ()` args-tuple) -> `ClosureCall` shim | positive | covered | `crates/bridge-coverage-runner/snippets/FF-017.rs` | regseq-driver-75: `let c = move \|\| x; c()` lowers to `<{closure} as Fn<()>>::call(move _self, ARGS)` where the optimized MIR folds the zero-arg args-tuple to a `const ()` operand. The closure-call rewrite `rewrite_closure_call_shim` only untupled `Move`/`Copy` place operands and otherwise bailed (`_ => Ok(None)`), so the call fell through to `translate_call_callee` -> `canonicalize_call_site_callee` (resolves the closure DefId) -> `callsite_fn_sig` -> `tcx.fn_sig(closure_def_id)`, which ICEs (`to get the signature of a closure, use args.as_closure().sig() not fn_sig()`). The fix handles the constant args-tuple inside the rewrite: an empty tuple contributes no positional args (`normalised` stays `[receiver]`, matching the `Fn<()>` shim arity); a non-empty all-ZST tuple (`c(())`) is materialised into a synthetic local via `Rvalue::Use` of the translated const operand and its fields projected, reusing the place-projection arm; and the catch-all bail is replaced with a typed `DslRejectionKind::Terminator` so no closure-call args shape can reach `tcx.fn_sig(closure)` again. The runner predicate is the coarse `SyntheticItemKind = ClosureCall` (same as IX-004 / SK-003); the discriminating signal is reaching `Ok`+validate AT ALL (HEAD ICEs). Test proof: `crates/regseq-driver/tests/bridge_zero_arg_closure_call.rs` (B1-B6 incl. FnMut, all-ZST arg, cloned-closure, and the non-ZST regression) |

## 2.4 `Body` structure axes (spec.md §25)

| id      | construct        | expectation | status      | fixture_path | notes |
| ------- | ---------------- | ----------- | ----------- | ------------ | ----- |
| BD-001  | Body with 1 BasicBlock (trivial) | positive | covered | `no_op.rs` | |
| BD-002  | Body with N>1 BasicBlocks (cfg) | positive | covered | many | from `if` / `match` |
| BD-003  | Body with arg_count > 0 | positive | covered | many | |
| BD-004  | Body with multiple LocalDecls of distinct types | positive | covered | many | |
| BD-005  | Body materialised for source-derived Function Instance | positive | covered | many | regseq-driver-60 |
| BD-006  | Body materialised for synthetic DropGlue Instance | positive | covered | `drop_glue_in_self_consuming_method.rs` | |
| BD-007  | Body materialised for synthetic ClosureCall Instance | positive | covered | `crates/bridge-coverage-runner/snippets/BD-007.rs` | regseq-driver-71 (post-driver-62 reattempt PASS; the prescan args-substitution bug IX-004 is closed at HEAD) |
| BD-008  | Body materialised for synthetic VTableMethodShim Instance | positive | covered | `trait_object.rs` | |
| BD-009  | Body with `body: None` for bucket-3 (generic) Item | positive | covered | `instance_completeness.rs` | regseq-driver-58 |
| BD-010  | Body with `body: None` for bucket-2 (foreign) Item | positive | covered | `foreign_rendering.rs` | |

---

# Section 3: Source-level feature axes

| id      | construct        | expectation | status      | fixture_path | notes |
| ------- | ---------------- | ----------- | ----------- | ------------ | ----- |
| FT-001  | Drop impl on a concrete type | positive | covered | `drop_glue_in_self_consuming_method.rs` | |
| FT-002  | Drop impl on a generic type (`impl<T> Drop for Wrapper<T>`) | positive | uncovered | -- | |
| FT-003  | Nested Drop fields (Drop type holding another Drop type, field-drop ordering) | positive | uncovered | -- | covers field-drop scheduling |
| FT-004  | `ManuallyDrop<T>` wrapping a Drop type | positive | uncovered | -- | NEGATIVE test for "drop is NOT scheduled" |
| FT-005  | Explicit `core::mem::drop(x)` call | positive | uncovered | -- | drop intrinsic call site |
| FT-006  | Trait declaration + impl + concrete call | positive | covered | `trait_method_resolution.rs` | regseq-driver-56 |
| FT-007  | Trait default method body | positive | covered | `trait_default_method.rs` | regseq-driver-60 |
| FT-008  | Trait default method body overridden by impl | positive | uncovered | -- | |
| FT-009  | Generic free fn called with concrete args | positive | uncovered | -- | needs driver-60 B4-style fixture |
| FT-010  | Generic struct + concrete instantiation | positive | partial | `instance_completeness.rs` (`Wrap<u32>`, `Wrap<u64>`) | the struct EXISTS in model.items; per-Item-instance body materialisation under monomorphisation needs verification |
| FT-011  | Closure with zero captures, called by ref (`impl Fn`) | positive | covered | `crates/bridge-coverage-runner/snippets/FT-011.rs` | regseq-driver-71 (post-driver-62 reattempt PASS) |
| FT-012  | Closure with N captures, called by ref | positive | covered | `crates/bridge-coverage-runner/snippets/FT-012.rs` | regseq-driver-71 (post-driver-62 reattempt PASS) |
| FT-013  | Closure called by mut ref (`impl FnMut`) | positive | covered | `crates/bridge-coverage-runner/snippets/FT-013.rs` | regseq-driver-71 (post-driver-62 reattempt PASS via the closure-call rewrite's `LangItem::FnMut -> ClosureCallKind::CallMut` arm) |
| FT-014  | Closure called by value (`impl FnOnce`) | positive | covered | `crates/bridge-coverage-runner/snippets/FT-014.rs` | regseq-driver-71 (post-driver-62 reattempt PASS via the closure-call rewrite's `LangItem::FnOnce -> ClosureCallKind::CallOnce` arm) |
| FT-015  | Trait object `&dyn Trait` static-method call | positive | covered | `trait_object.rs` | |
| FT-016  | Trait object `Box<dyn Trait>` | rejected | blocked-bridge-rejects-by-design | -- | cross-ref Section 6 row RJ-001 |
| FT-017  | `const fn` evaluated in const context | positive | partial | playground (`Reg::REG_0`) | |
| FT-018  | `const FOO: T = ...;` const item | positive | covered | `instance_completeness.rs`, playground | |
| FT-019  | Associated const on an `impl` block | positive | covered | playground | |
| FT-020  | `#[rseq::intrinsic("log_*")]` recognition | positive | covered | `diag_logging.rs` | |
| FT-021  | Diagnostic-log format-template parse rejection | rejected | partial | -- | Section 6 RJ-031 sub-axes |

---

# Section 4: Compilation / toolchain axes

| id      | construct        | expectation | status      | fixture_path | notes |
| ------- | ---------------- | ----------- | ----------- | ------------ | ----- |
| CT-001  | `panic = "abort"` (required for phase 1a per `bridge/body/basic_block.rs:23`) | positive | covered | every fixture via helper (`crates/test-ir-fixtures/src/lib.rs:188-198`) | |
| CT-002  | `panic = "unwind"` (default) -- bridge rejection | rejected | covered (rejection-path covered) | Section 6 RJ-008 cross-ref | `basic_block.rs:25` |
| CT-003  | Edition 2021 | positive | covered | every fixture | |
| CT-004  | Edition 2024 | positive | uncovered | -- | |
| CT-005  | `#![feature(register_tool)] #![register_tool(rseq)]` | positive | covered | `diag_logging.rs` only (other fixtures are plain `#![no_std]`) | |
| CT-006  | `#![no_std]` topology (no alloc, no std) | positive | covered | every fixture | |
| CT-007  | `#![no_std] extern crate alloc;` (alloc dep) | rejected | blocked-bridge-rejects-by-design | -- | `DslRejection::Box`-like |
| CT-008  | Multiple crates in workspace | positive | partial | implicit | bridge tests primary crate only |
| CT-009  | `#[inline(never)] #[rseq::intrinsic(...)]` attribute (intrinsic recognition) | positive | covered | `diag_logging.rs` | |
| CT-010  | `#[link_section = "..."]` / `#[no_mangle]` / `#[linkage = "weak"]` per `Linkage` enum (4 variants per `item.rs:138`) | positive | partial | `Linkage::Default` ubiquitous; `NoMangle` / `Foreign` covered via `foreign_rendering.rs`; `Weak` uncovered | |
| CT-011  | Generic-args width: 1 type-param vs N | positive | partial | `instance_completeness.rs` (`Wrap<T>` -- 1 type-param) | N>1 uncovered |
| CT-012  | Lifetime parameters (erased at bridge) | positive | covered | `diag_logging.rs` (`LogArg<'_>`) | |

---

# Section 5: Interaction matrix -- HIGH BUG-DISCOVERY VALUE

Per Codex review (round 1): the interaction matrix is the load-bearing
section for bug discovery. Each row maps to either a covered fixture
or, if uncovered, the bug class it would surface.

| id      | interaction | expectation | status | fixture_path / bug-task | notes |
| ------- | ----------- | ----------- | ------ | ----------------------- | ----- |
| IX-001  | GenericArgs × Direct Call (generic fn called with concrete args, fn body materialises) | positive | uncovered | -- | driver-60 B4 target; latent generalisation of driver-60 |
| IX-002  | Trait default body × Body materialisation (substitution at trait DefPath) | positive | covered | `trait_default_method.rs` | driver-60 regression guard |
| IX-003  | Drop impl × FnSig interning (Drop-as-Call collapse parallel-table sync) | positive | covered | `drop_glue_in_self_consuming_method.rs` | driver-61 regression guard |
| IX-004  | Closure (`impl Fn`) × `Fn::call` site translation (polymorphic body containing `<F as Fn>::call`) | positive | covered | `crates/bridge-coverage-runner/snippets/IX-004.rs` | regseq-driver-71 (post-driver-62 reattempt PASS) |
| IX-005  | Closure × FnSig parallel-table sync (eager synth path) | positive | covered | `closure_call_sig_sync.rs` | driver-61 regression guard |
| IX-006  | FnPtr × FnSig parallel-table sync (Rust + C ABI dispatch on `FnAbi::C`) | positive | covered | `fn_ptr_translation_sig_sync.rs` | driver-61 regression guard |
| IX-007  | `dyn Trait` × Drop type (vtable drop-in-place shim, SK-011) | positive | uncovered | -- | exercises VTableDropShim path |
| IX-008  | `dyn Trait` × generic-method-call substitution at vtable shim site | positive | uncovered | -- | substitution at VTableMethodShim |
| IX-009  | Self-consuming method × Drop (on-success-path drop terminator) | positive | covered | `drop_glue_in_self_consuming_method.rs` | driver-61 firing case |
| IX-010  | Generic fn × Trait method call inside body (polymorphic `Fn::call` / trait-method resolve) | positive | covered | `crates/bridge-coverage-runner/snippets/IX-010.rs` | regseq-driver-71 (post-driver-62 reattempt PASS) |
| IX-011  | Trait default method × GenericArgs substitution at body materialisation | positive | covered | `trait_default_method.rs` | driver-60 |
| IX-012  | Nested Drop type × on-success-path drop (field-drop ordering) | positive | uncovered | -- | drop scheduling order |
| IX-013  | Closure × Captures with Drop type | positive | uncovered | -- | exercises ClosureDropGlue (SK-006) |
| IX-014  | Closure × `FnOnce` consumed via Move | positive | covered | `crates/bridge-coverage-runner/snippets/IX-014.rs` | regseq-driver-71 PASS via closure-call rewrite to `ClosureCall { call_kind: CallOnce }`; the embedded FF-007 ClosureOnceShim sub-axis remains uncovered (separate Phase-5 `ItemMissingInstance` bridge gap surfaced by F71), and the SK-005 CallOnce sub-axis is masked by the runner predicate-precision gap |
| IX-015  | Generic fn × Generic struct field access | positive | uncovered | -- | substitution in `Place::Field` |
| IX-016  | Const eval × Const item used in body | positive | covered | playground | |
| IX-017  | Diag-log intrinsic × generic caller | positive | uncovered | -- | log call inside `pub fn f<T>(t: T)` |
| IX-018  | Validator invariant `sig_c_abi_lowerings.len() == sigs.len()` × Drop-as-Call FnSig synth | positive | covered | implicit via driver-61 debug_assert | |
| IX-019  | Two adjacent end-of-scope drops (reverse-declaration-order scheduling) | positive | uncovered | -- | playground demonstrated; needs committed test |
| IX-020  | Drop impl + assoc consts + inherent methods on same struct | positive | covered | playground (uncommitted user fixture) | |
| IX-021  | `dyn Trait + Send` (auto-trait combination) | positive | uncovered | -- | exercises TY-014-B |
| IX-022  | `Const::Reloc` (function-pointer literal) × call-site translation | positive | uncovered | -- | exercises OP-003-K + RV-005-C |
| IX-023  | Diverging call (`Call.target == None`) × subsequent block reachability analysis | positive | uncovered | -- | exercises TM-005-D |
| IX-024  | `panic = "abort"` MIR rewrite × cleanup-block-bearing source (Drop on success path) | positive | covered | `drop_glue_in_self_consuming_method.rs` | |
| IX-025  | `Cast::Unsize` × vtable instance synthesis at use-site | positive | covered | `instance_completeness.rs`, `trait_object.rs` | |

### Notes on the open bug (IX-004 / IX-010) -- historical

Before regseq-driver-62 (commit at
`.harness/expectations/regseq-driver-62.md:51-68`), the bridge
failed on:

```rust
fn apply(x: i32, y: i32, f: impl Fn(i32, i32) -> i32) -> i32 { f(x, y) }
pub fn entry(x: i32, y: i32) -> i32 { apply(x, y, |x, y| x + y) }
```

because the Phase-2 synthesis pre-scan at
`crates/regseq-driver/src/bridge/mod.rs:769` walked
`tcx.optimized_mir(local_def_id)` (the POLYMORPHIC body), not the
substituted body, then attempted to resolve the polymorphic
`<F as Fn>::call` callee via `Instance::try_resolve`. Same
root-cause shape as driver-60 (process polymorphic MIR without
applying `instance.args`), different pass.

**regseq-driver-62 landed the fix**: HEAD's
`crates/regseq-driver/src/bridge/mod.rs:769-789` now calls
`tcx.instance_mir(instance.def)` plus
`instantiate_mir_and_normalize_erasing_regions(...)`, processing the
substituted body. The minimal repro above now translates cleanly
through Phase 2.

**regseq-driver-71** (the closure-family reattempt sweep + this
refresh) re-verified the cohort that was previously blocked by this
bug. The per-row reattempt outcomes are recorded in
`.harness/expectations/regseq-driver-71.md`. The row-status table
entries that previously read `blocked-open-bridge-bug` are
reclassified per those outcomes:

- **Rows that PASS** become `covered`.
- **Rows that escalate against a SEPARATE bridge gap** (e.g., the
  Fn-as-FnOnce coercion path's Phase-5 `ItemMissingInstance`
  rejection F71 surfaced for FF-007, or the RustCall ABI rejection
  F70 surfaced for FF-005 at
  `crates/regseq-driver/src/bridge/fnsig.rs:107-115`) get a status
  note pointing at the new bridge-side follow-up.
- **Rows that escalate against a runner-side predicate-precision
  gap** (the missing `ClosureCallKind` sub-axis in
  `InspectionPredicate::SyntheticItemKind` at
  `crates/bridge-coverage-runner/src/inspect.rs:50-53` +
  `:168-174` that prevents distinguishing CallMut / CallOnce from
  Call -- SK-004 / SK-005) get a status note pointing at the F2
  IX-014 follow-up direction recorded at
  `.harness/expectations/regseq-driver-65.md:238-240`.

The `IX-004` and `IX-010` row entries themselves are updated below;
downstream rows are updated in their own sections.

---

# Section 6: Intentional rejection rows (negative coverage)

Per Codex review (round 1): each `BridgeError` variant defined at
`crates/regseq-driver/src/bridge/mod.rs:376-...` gets a row. Sub-enums
(`CAbiTypePosition`, `DiagLogFormatReason`, `DiagLogValidation`'s
diagnostic IDs) get first-class sub-axes per the round-1 follow-up.

Failures here would be either PHASE-ERR (rejection surfaced but with
wrong classification) or VALIDATOR-FAIL.

## 6.1 `BridgeError::DslRejection` (kind: `DslRejectionKind`, 8 variants per `bridge/mod.rs:580`)

| id      | rejection variant | source trigger | status | notes |
| ------- | ----------------- | -------------- | ------ | ----- |
| RJ-001  | `DslRejectionKind::Box` | `Box<T>` field or local | rejected | uncovered |
| RJ-002  | `DslRejectionKind::CVoid` | `core::ffi::c_void` use | rejected | uncovered |
| RJ-003  | `DslRejectionKind::Atomic` | `core::sync::atomic::AtomicU32` etc. | rejected | uncovered |
| RJ-004  | `DslRejectionKind::PlaceProjection` | unsupported projection in body | rejected | uncovered (no known surface from `#![no_std]` source) |
| RJ-005  | `DslRejectionKind::Rvalue` | unsupported rvalue (e.g., `ThreadLocalRef`) | rejected | uncovered |
| RJ-006  | `DslRejectionKind::Statement` | unsupported statement | rejected | uncovered |
| RJ-007  | `DslRejectionKind::Terminator` | unsupported terminator (e.g., `Yield`) | rejected | uncovered (requires coroutine source -- blocked by RJ-016) |
| RJ-008  | `DslRejectionKind::BasicBlock` (`is_cleanup=true`) | `panic = "unwind"` compile | rejected | covered (implicitly via helper enforcing panic=abort; would fire under default panic) |

## 6.2 `BridgeError::NonRigidTy` (kind: `NonRigidTyKind`, 6 variants per `bridge/mod.rs:601`)

| id      | rejection variant | source trigger | status | notes |
| ------- | ----------------- | -------------- | ------ | ----- |
| RJ-009  | `NonRigidTyKind::Param` | bucket-4 carve-out (`signature_non_rigid`) via non-rigid RPIT signature | covered | regseq-driver-72 sub-task C path C.2.a.i (committed as a positive row in `rows.toml`). Reframed from "Param rejection" to positive-row coverage of the bridge bucket-4 carve-out at `crates/regseq-driver/src/bridge/mod.rs:1644-1658`. Semantic shift rationale: any `Param`-bearing signature makes the Item generic, which the four-bucket validator routes to bucket 3 (skipped at `bridge/mod.rs:1584`) before the carve-out, and trait-method `Self` Params are skipped at `bridge/mod.rs:1600-1606`; the carve-out flag is also kind-agnostic (records `signature_non_rigid: bool`, not `Param` vs `Alias`), so no source shape surfaces a `NonRigidTyKind::Param` rejection through the current oracle (F71's 3 probes + F72's 4th probe confirm this). The committed snippet `crates/bridge-coverage-runner/snippets/RJ-009.rs` (private uncalled `-> impl Fn(i32) -> i32`) drives the carve-out's canonical `Alias(Opaque)` path, setting `signature_non_rigid = true`; the new `SignatureNonRigid` inspection predicate reads that flag from the in-process model. Full disposition in `.harness/expectations/regseq-driver-72.md` |
| RJ-010  | `NonRigidTyKind::Bound` | bound generic surviving | rejected | uncovered |
| RJ-011  | `NonRigidTyKind::Placeholder` | placeholder surviving | rejected | uncovered |
| RJ-012  | `NonRigidTyKind::Infer` | infer surviving | rejected | uncovered |
| RJ-013  | `NonRigidTyKind::Alias` | unresolved alias | rejected | uncovered |
| RJ-014  | `NonRigidTyKind::Error` | rustc Error type | rejected | blocked-rustc-infeasible | rustc rejects first |

## 6.3 Top-level rejection variants (per `bridge/mod.rs:376-508`)

| id      | rejection variant | source trigger | status | notes |
| ------- | ----------------- | -------------- | ------ | ----- |
| RJ-015  | `Foreign` | non-extern-block foreign type | rejected | uncovered |
| RJ-016  | `Coroutine` | `async fn` or generator | rejected | uncovered |
| RJ-017  | `Pat` | refinement pattern types | rejected | blocked-rustc-infeasible | feature gated |
| RJ-018  | `UnsafeBinder` | unsafe-binder Ty | rejected | uncovered |
| RJ-019  | `DynAutoOnly` | `dyn Send + Sync` (no principal) | rejected | uncovered |
| RJ-020  | `DynStar` | `dyn*` type | rejected | blocked-rustc-infeasible | feature gated |
| RJ-021  | `CustomAutoTrait` | source-declared auto trait | rejected | blocked-rustc-infeasible | feature gated |
| RJ-022  | `SubstantiveHrtb` | substantive HRTB binder | rejected | uncovered |
| RJ-023  | `Layout` | uncomputable layout | rejected | uncovered |
| RJ-024  | `UnsupportedEnumLayout` | enum with unsupported tag scalar | rejected | blocked-rustc-infeasible | The three rejection paths (`Scalar::Union` tag `ty.rs:1320`, `Float` tag `ty.rs:1368`, niche-out-of-range `ty.rs:1446`) are defensive bridge invariants for layouts rustc's solver does not emit from source: enum discriminants are integral (never Float/Union tags) and niche ranges respect rustc's guarantees. Probed niche / `repr(u128)` / niche-squeeze enums — none hit it. See `.harness/audits/2026-05-23-b2-deferred-shim-probe.md` (B2) |
| RJ-025  | `UnsupportedTy` | unrecognised rustc TyKind | rejected | uncovered |
| RJ-026  | `UnsupportedAbi` | non-Rust / non-C ABI | rejected | uncovered |
| RJ-027  | `UnsupportedNeverInput` | `!` as fn input | rejected | uncovered |
| RJ-028  | `UnsupportedCAbiType` -- per `CAbiTypePosition` (3 variants per `bridge/mod.rs:364`) | rejected | partial | per-position sub-axes below |
| RJ-029  | `UnsupportedConst` | unrecognised const shape | rejected | uncovered |
| RJ-030  | `GenericOpaqueHandle` | generic `#[rseq::opaque]` | rejected | uncovered |
| RJ-031  | `DiagLogFormat` -- per `DiagLogFormatReason` (6 variants per `bridge/mod.rs:535`) | rejected | partial | per-reason sub-axes below |
| RJ-032  | `DiagLogValidation` -- per diagnostic ID (6 variants per `bridge/mod.rs:556`) | rejected | partial | per-diagnostic sub-axes below |

### RJ-028 sub-axes (`CAbiTypePosition`)

| id        | sub-construct | status | notes |
| --------- | ------------- | ------ | ----- |
| RJ-028-A  | `CAbiTypePosition::Input(usize)` | uncovered | non-C-ABI type in `extern "C"` fn input |
| RJ-028-B  | `CAbiTypePosition::Output` | uncovered | non-C-ABI type in `extern "C"` fn output |
| RJ-028-C  | `CAbiTypePosition::Field` | uncovered | non-C-ABI type in `#[repr(C)]` struct field |

### RJ-031 sub-axes (`DiagLogFormatReason`)

| id        | sub-construct | status | notes |
| --------- | ------------- | ------ | ----- |
| RJ-031-A  | `DebugFormatterRejected` (`{:?}`) | uncovered | log call with `{:?}` template |
| RJ-031-B  | `PositionalArgRejected` (e.g., `{0}`) | uncovered | |
| RJ-031-C  | `NamedArgRejected` (e.g., `{name}`) | uncovered | |
| RJ-031-D  | `NonSubsetModifier` (e.g., `{:.3}`) | uncovered | |
| RJ-031-E  | `UnclosedBrace` | uncovered | |
| RJ-031-F  | `UnmatchedClosingBrace` | uncovered | |

### RJ-032 sub-axes (`DiagLogValidation` diagnostic IDs)

| id        | sub-construct | status | notes |
| --------- | ------------- | ------ | ----- |
| RJ-032-A  | `D1` | uncovered | |
| RJ-032-B  | `D2` | uncovered | |
| RJ-032-C  | `D4` | uncovered | |
| RJ-032-D  | `D6` | uncovered | |
| RJ-032-E  | `D7` | uncovered | |
| RJ-032-F  | `D8` | blocked-unimplemented-phase | D8 short-circuits to `Ok(())` because `target_str_arg_bound` (`bridge/diag_log/peel.rs:179-180`) is hard-wired `None` pending the backend target-binding wiring that populates `max_str_args_per_call` (spec §26.8/26.9 + per-target `<target>-backend.md`). Unreachable from source until that phase lands. See `.harness/audits/2026-05-23-f1-rj032f-watchlist.md` (F1) |

---

# Section 7: Validator invariants (validator-failing expectations)

Every `InvariantViolation` variant defined at
`crates/regseq-model/src/model.rs:163-831` gets a row -- **61 variants**
total, sourced via `awk`/grep against the enum block. (Was 60 until the
regseq-driver-78 audit added `DuplicateInstanceKey`, enum variant #12, which
the regseq-driver-73/R1 hardening introduced after Section 7 was first
authored; the ids below were renumbered to restore enum-order
correspondence.) Validator-failing fixtures are CONSTRUCTED-MODEL tests, not
source-level Rust fixtures.

The `exercised?` column was verified by the regseq-driver-78 audit
(`.harness/audits/2026-05-25-section7-validator-audit.md`): for each variant,
a genuine asserting test (construct a defective `RegseqModel` -> call
`validate()` / `validate_bridge_completeness()` -> assert *that* variant) was
located, or the variant marked `no`. The audit corpus is **six** test files,
not the one this header originally named:
`id_referential_integrity_validator.rs`, `layout_validator.rs`,
`place_validator.rs`, `validate_bridge_completeness_buckets.rs`,
`validate_item_instance_coverage.rs` (the five external
`crates/regseq-model/tests/` files), plus the in-crate
`crates/regseq-model/src/model_tests.rs` module
(`#[cfg(test)] #[path] mod tests;` at `model.rs:121-123`). The `notes` column
cites the exercising test for `yes`/`partial`, or the fixture-form +
audit-doc cross-reference for `no`. See the audit doc for per-variant
`file:fn` citations and defect sketches.

| id      | variant | exercised? | notes |
| ------- | ------- | ---------- | ----- |
| VL-001  | `DanglingId` | yes | `model_tests.rs:invariant_1_dangling_item_id_in_instance` + `id_ref` dangling/const/span tests |
| VL-002  | `ItemDefPathsLengthMismatch` | yes | `model_tests.rs:invariant_2_item_def_paths_length_mismatch` |
| VL-003  | `ItemSyntheticKindsLengthMismatch` | yes | `id_ref:invariant_2_reject_synthetic_kinds_short`; `model_tests.rs:invariant_2_item_synthetic_kinds_length_mismatch` |
| VL-004  | `PrimaryCrateOutOfRange` | yes | `model_tests.rs:invariant_4_primary_crate_out_of_range` |
| VL-005  | `PrimaryCrateStableIdMismatch` | yes | `model_tests.rs:invariant_4_primary_crate_stable_id_mismatch` |
| VL-006  | `DuplicateStableCrateId` | yes | `model_tests.rs:invariant_5_duplicate_stable_crate_id` |
| VL-007  | `DefPathCrateIdOutOfBounds` | yes | `id_ref:invariant_6_reject_def_path_crate_id_out_of_bounds` |
| VL-008  | `DuplicateInternerEntry` | yes | `id_ref:invariant_11_{ty,fn_sig,const}_interner_pass_and_reject_duplicate`, `phase_b_then_c_ordering` |
| VL-009  | `DuplicateItemDefPathHash` | yes | `id_ref:invariant_18_reject_duplicate_item_def_path_hash` |
| VL-010  | `F4OutOfOrder` | yes | `id_ref:f4_sort_reject_permuted_input` |
| VL-011  | `F4OutOfOrderInstance` | yes | `id_ref:f4_2_sort_reject` |
| VL-012  | `DuplicateInstanceKey` | yes | NEW row (enum #12). `validate_bridge_completeness_buckets.rs:bucket1_function_item_with_duplicate_shim_instances_rejected` |
| VL-013  | `DefPathRenderMismatch` | yes | `id_ref:def_path_render_reject_malformed` |
| VL-014  | `SpanOutOfBounds` | yes | `id_ref:span_bounds_reject_lo_gt_hi`, `span_bounds_reject_hi_gt_content_len` |
| VL-015  | `SpanNotOnUtf8Boundary` | yes | `id_ref:span_bounds_reject_not_on_utf8_boundary_{lo,hi}` |
| VL-016  | `DuplicateVTablePrincipal` | yes | `id_ref:phase_c_vtables_rebuild_reject_duplicate_principal` |
| VL-017  | `SynthesizedSentinelInvalid` | yes | `model_tests.rs:invariant_8_synthesized_sentinel_{invalid_path,with_content_rejected,with_wrong_line_starts_rejected}` |
| VL-018  | `StringInternerSentinelInvalid` | no | unreachable-by-construction backstop: `StringInterner` encapsulation (private fields; `new()`/`intern()` preserve the sentinel; `Deserialize` rejects non-empty index 0) makes the defect unrepresentable without a test-only hook. Not a fixture-batch task. See audit doc S.2/S.3 finding 5 |
| VL-019  | `DuplicateString` | no | unreachable-by-construction backstop: `intern()` is content-deduplicating and `Deserialize` rejects duplicates (`string_interner.rs:49-50,157-161`), so the defect needs a test-only hook. Not a fixture-batch task. See audit doc S.2/S.3 finding 5 |
| VL-020  | `PlaceWalk` | yes | `id_ref:place_validator_structural_error_at_index_5_not_masked_by_type_error_at_index_0`; `place_validator.rs:joint_validator_integration_pass_and_reject` |
| VL-021  | `SyntheticKindItemKindMismatch` | no | constructed-model gap; `ItemKind::Function` item with `SyntheticItemKind::ClosureCaptures` (requires `Struct`). See audit doc S.2 |
| VL-022  | `DirectCalleeOutOfRange` | yes | `id_ref:terminator_b7_direct_callee_out_of_range_rejects`; `place_validator.rs:joint_validator_rejects_dangling_direct_call_instance` |
| VL-023  | `TargetMalformed` | yes | `layout_validator.rs:target_reject_*` (6 reasons) |
| VL-024  | `BuildMalformed` | yes | `layout_validator.rs:build_reject_toolchain_empty` (one of two reasons; `reason`-string) |
| VL-025  | `FnSigMalformed` | yes | `layout_validator.rs:fn_sig_c_variadic_reject_{non_c_abi,zero_inputs}`, `fn_sig_reject_inputs_never` |
| VL-026  | `CAbiLoweringMalformed` | yes | `layout_validator.rs:cabi_lowering_reject_*` (5 tests) |
| VL-027  | `LayoutMalformed` | yes | `layout_validator.rs:layout_reject_field_{overlap,offset_misaligned,overflows_layout_size}` (struct-kind reasons; tuple/array/union/enum untested -- same variant) |
| VL-028  | `TagScalarOutOfRange` | yes | `layout_validator.rs:tag_scalar_width_fit_reject_overflow` |
| VL-029  | `InstanceLayoutMismatch` | yes | `layout_validator.rs:invariant_14_reject_kind_mismatch_struct_to_enum` |
| VL-030  | `InstanceLayoutItemArgsMismatch` | yes | `layout_validator.rs:invariant_14_reject_item_args_mismatch` |
| VL-031  | `ConstGraphCycle` | yes | `layout_validator.rs:const_graph_cycle_reject_{self_loop,two_node,enum_value_payload}` |
| VL-032  | `DomainAttrMalformed` | yes | `layout_validator.rs:domain_attr_*_reject*`, `intrinsic_singleton_reject_two_intrinsics` (6 tests) |
| VL-033  | `OpaqueHandleInconsistent` | yes | `layout_validator.rs:invariant_19_reject_{non_struct_kind,generic,non_zst_layout,by_value_use}` |
| VL-034  | `StableCrateIdZeroCollision` | yes | `layout_validator.rs:invariant_20_reject_real_crate_zero` |
| VL-035  | `SyntheticDomainBoundary` | yes | `layout_validator.rs:invariant_20_reject_{source_derived_synthetic_domain,synthetic_non_zero_domain}` |
| VL-036  | `UnitLayoutMismatch` | yes | `layout_validator.rs:unit_layout_reject_non_canonical` |
| VL-037  | `SourceFileMalformed` | yes | `layout_validator.rs:source_file_reject_line_start{s_non_monotonic,s_does_not_start_at_zero,_exceeds_content_len}` |
| VL-038  | `DuplicateSourceFilePath` | yes | `layout_validator.rs:source_file_reject_duplicate_path` |
| VL-039  | `BodyTyIdOutOfRange` | yes | `id_ref:body_ty_closure_rejects_dangling_local_decl_ty` |
| VL-040  | `StatementAssignTypeMismatch` | yes | `id_ref:statement_b1_assign_type_mismatch_rejects` |
| VL-041  | `StatementAssignMalformed` | partial | `id_ref:statement_b1_assign_malformed_rejects_ptr_metadata_on_non_fat_pointer` asserts only `side="rvalue"`; `side="place"` arm untested. See audit doc S.2 |
| VL-042  | `SetDiscriminantVariantIdxOutOfRange` | yes | `id_ref:statement_b2_set_discriminant_variant_idx_out_of_range_rejects` |
| VL-043  | `SetDiscriminantNonEnumTarget` | yes | `id_ref:statement_b3_set_discriminant_non_enum_target_rejects` |
| VL-044  | `AssumeOperandNotBool` | yes | `id_ref:statement_b4_assume_operand_not_bool_rejects` |
| VL-045  | `CopyNonOverlappingOperandType` | partial | `id_ref:statement_b5_copy_non_overlapping_count_type_rejects` asserts only `which="count"`; `src`/`dst`/`pointee` arms untested. See audit doc S.2 |
| VL-046  | `SwitchIntDiscrTypeMismatch` | yes | `id_ref:terminator_b1_switch_int_discr_type_mismatch_rejects` |
| VL-047  | `AssertCondNotBool` | yes | `id_ref:terminator_b2_assert_cond_not_bool_rejects` |
| VL-048  | `CallDestinationTypeMismatch` | yes | `id_ref:terminator_b3_call_destination_type_mismatch_rejects{,_indirect}` |
| VL-049  | `CallArgsArityMismatch` | yes | `id_ref:terminator_b4_call_args_arity_mismatch_rejects_{direct,indirect}`; `model_tests.rs:{non_log_direct_call_uses_generic_b4,indirect_call_does_not_receive_log_exception}` |
| VL-050  | `CallDirectCalleeNotFunction` | yes | `id_ref:terminator_b5_call_direct_callee_not_function_rejects` |
| VL-051  | `CallIndirectCalleeNotFnPtr` | yes | `id_ref:terminator_b5_call_indirect_callee_not_fn_ptr_rejects` |
| VL-052  | `CallArgTypeMismatch` | yes | `id_ref:terminator_b6_call_arg_type_mismatch_rejects_{direct,indirect}` |
| VL-053  | `CallVirtualIdxInconsistent` | yes | `id_ref:terminator_b8_virtual_idx_inconsistent_rejects` |
| VL-054  | `CallTargetDivergenceMismatch` | yes | `id_ref:terminator_b9_target_divergence_mismatch_rejects_{non_diverging_no_target,indirect_divergent_target_some}` |
| VL-055  | `DiagLogCallArg0NotStrRef` | yes | `model_tests.rs:rejects_args_zero_not_str_ref` |
| VL-056  | `DiagLogCallArityBelowMinimum` | yes | `model_tests.rs:rejects_empty_args` |
| VL-057  | `DiagLogCallArgNotAdmissible` | yes | `model_tests.rs:rejects_args_one_not_admissible_log_arg_source` |
| VL-058  | `ItemMissingInstance` | yes | `validate_bridge_completeness_buckets.rs:bucket{1_struct,2_foreign_function}_zero_instances_rejected`; `validate_item_instance_coverage.rs:b{1,2}_*_without_instance_is_rejected` |
| VL-059  | `Bucket1MultipleInstances` | yes | `validate_bridge_completeness_buckets.rs:bucket1_{struct_multiple_instances,function_item_with_two_primary_instances}_rejected` |
| VL-060  | `ForeignInstanceBodyOrInitNotNone` | yes | `validate_bridge_completeness_buckets.rs:bucket2_foreign_{function_body,static_init}_must_be_none` |
| VL-061  | `Bucket4HasInstance` | yes | `validate_bridge_completeness_buckets.rs:bucket4_rpit_function_with_instance_rejected` |

Audit roll-up (`.harness/audits/2026-05-25-section7-validator-audit.md`):
coverage axis **56 `yes`, 2 `partial` (VL-041, VL-045), 3 `no` (VL-018,
VL-019, VL-021)**; triage axis **1 fixture-bound `no` (VL-021), 2
`unreachable-by-construction` backstops (VL-018, VL-019), 0 dead**, over all
61. The fixture-batch follow-up is **3 gaps** -- VL-021 (constructed-model)
plus completing the two `partial`s -- grouped by enforcing method
(`validate_synthetic_kinds`, `validate_copy_non_overlapping`,
`validate_body_statement_invariants`); not authored here. VL-018/VL-019 are
excluded from that batch (they need a test-only `StringInterner` hook, a
source change, not a model-level fixture).

---

# Section 8: Open questions for next review round

1. **Closure capture × call-kind sub-axes** (FT-012/013/014, SK-004/005,
   FF-007, IX-013/014): the `Fn`/`FnMut`/`FnOnce` × capture-count
   cross-product is 3 x N. Should the matrix list each individually,
   or bundle them under "closure-call shape coverage" with worked
   examples?
2. **Drop ordering rows** (FT-002, FT-003, IX-012, IX-019): drop
   scheduling is order-sensitive; separate row per ordering scenario,
   or one row per "scheduling shape" with worked examples?
3. **`dyn Trait + auto-traits` exhaustiveness** (TY-014-A..E,
   IX-021): the auto-trait power-set is 2^3 = 8 combinations. Is each
   a row, or sample representative subsets?
4. **Sub-axes formalism**: this v2 promotes sub-axes to first-class
   rows (e.g., RV-005-A..N, RV-006-A..Z). Is the resulting
   ~250-300-row matrix the right granularity, or should sub-axes
   collapse back to "covers X via fixture Y" notes?
5. **Lifecycle of "uncovered" rows**: auto-tick on first fixture
   land, or require a matrix-update PR alongside each fixture?
6. **Section 7 verification cadence**: each `unknown -- verify` cell
   requires reading
   `crates/regseq-model/tests/id_referential_integrity_validator.rs`
   and any sibling validator tests. Should this be one task ("audit
   validator-test coverage and update Section 7") or rolled into
   each fixture-batch task?
7. **Coverage estimate after v2** (recounted per Codex round-2
   audit): ~359 total table rows across Sections 1-6 (299 non-VL,
   excluding the 60 validator-invariant rows in Section 7).
   Breakdown of non-VL rows: ~100 covered, ~133 uncovered, ~20
   partial, ~41 rejected (Section 6), ~5 blocked-* (other than
   rejected). Open-bridge-bug rows: 3 (IX-004, IX-010, BD-007,
   FT-011 -- collapsed via cross-references). Uncovered batch is
   ~133 fixture-authorable rows (plus the Section 7 verify-and-
   fill follow-up). Phasing implications: the fixture-authoring
   batch needs to be broken into sub-batches by topic, not a
   single approved brief.

---

# Status change log

- **v2.1** (current): addressed Codex round-2 minor fixes:
  - Fixed OP-003-I `Const::EnumValue` payload shape:
    `{ variant_idx, payload }` (not `{ variant, payload, ty }`)
    per `crates/regseq-model/src/const_value.rs:41-47`.
  - Demoted TY-001 (`Ty::Bool`) from `partial` to `uncovered`;
    earlier `trait_object.rs` fixture claim was unsupported
    (the fixture only uses `u8`/dyn-reader surface).
  - Replaced the speculative ~140/~120/~20/~15/~3 coverage
    estimate with the Codex-audited recount (~100 covered /
    ~133 uncovered / ~20 partial / ~41 rejected / ~5 blocked-*
    out of ~299 non-VL rows; Section 7 verify-and-fill follow-up
    separate).

- **v2** (initial revision): addressed Codex round-1 findings:
  - Fixed schema mismatches: `Ty::Ref(_, Provenance)`,
    `Ty::Dynamic { auto_traits, vtable_ty }` (no `projections`),
    `PlaceElem::Downcast(VariantIdx)`, removed spurious `TM-007`
    Drop-as-terminator row.
  - Promoted sub-axes to first-class rows: `FloatKind` (4),
    `CastKind` (11) including `PointerCoercion` (4) inline,
    `BinOp` (26), `UnOp` (3), `AggregateKind` (7),
    `Const` (11) including `Byte` (3) inline, `AssertKind` (8),
    `Provenance` (2), `CAbiTypePosition` (3), `DiagLogFormatReason`
    (6), `DiagLogValidation` diagnostic IDs (6),
    `IntKind`/`UintKind` (6 each).
  - Fixed fixture inventory: `instance_completeness.rs` covers
    static / static mut / const / union / generic struct
    (corrected IK-002/003/004/007, TY-019); `no_op.rs` is bare-
    minimum (TY-001 demoted to partial); `register_tool(rseq)` is
    `diag_logging.rs` only (CT-005 corrected).
  - Added `partial`, `blocked-open-bridge-bug` to declared status
    taxonomy.
  - Dropped pass-timing rows PT-001..PT-005 (positive-by-
    construction; not fixture-authorable) and PHASE-ERR rows
    PE-001..PE-004 from the fixture backlog.
  - Added missing dimensions: generic-args width (CT-011), lifetime
    parameters (CT-012), `Linkage` enum sub-axes (CT-010), `Call`
    divergence sub-axes (TM-005-C/D), `dyn Trait + auto-traits`
    combinations (TY-014-A..E), `Provenance` and `Mutability` on
    refs/pointers (TY-010-A/B, TY-011-A/B).
  - Expanded Section 7 to all 60 `InvariantViolation` variants
    (per `awk` against the enum block at
    `crates/regseq-model/src/model.rs:163`); marked unknown rows
    `verify` for follow-up cross-check against
    `id_referential_integrity_validator.rs`.
