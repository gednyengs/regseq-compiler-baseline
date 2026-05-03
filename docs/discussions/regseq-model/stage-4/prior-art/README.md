# Stage-4 Prior-Art Synthesis

Cross-cluster summary across 7 projects examined for the const-
vocabulary problem (D-4-1 onward). Each per-project file (cited
inline below as `<project>.md`) carries findings against 8
sub-questions covering variant set, type-tag placement,
aggregates, relocations, null pointers, uninit/padding, validation
phase, and cross-build determinism.

This README distils what the cluster says together: convergent
patterns we should adopt, real forks where projects diverge,
genuinely novel territory, patterns to avoid, new ideas surfaced.
Section 6 maps each finding to D-4-1's framing.

## 1. Convergent patterns (adoption candidates)

### 1.1 Inline scalar type-tag (3-of-7 confirm; spec.md §9.7 already pins)

`Int(IntKind, value)` style — the type tag travels with the value
inside the variant.

- **Confirm**: spec.md §9.7 (already pinned), `mlir.md`
  (`TypedAttrInterface`), `mrustc.md` (`MIR::Constant`).
- **Hybrid (inline + outer wrapper)**: `charon.md`
  (`ConstantExpr { kind, ty }` carries outer `ty: Ty` AND inline
  `ScalarValue::Signed(IntTy, _)`), `regseq-model-old.md`
  (`Constant.ty: RegseqType` + inline width tag), `compcert.md`
  (`init_data` family is inline; `Csyntax.Eval (val, type)` adds
  outer).
- **Externalised (no inline tag)**: `llvm.md` (type lives on
  inherited `Value::Type *`), `rustc.md` (type carried at the
  enclosing pair, `mir::Const::Val(ConstValue, Ty)`).

The split is clean: pure-externalised is rare; pure-inline and
hybrid both have multiple votes. The outer-wrapper question is a
real fork — see §2.1.

### 1.2 Per-pointee-type typed null (LLVM-strong)

`NullPtr(TyId)` — null pointers carry pointee type for canonical
equality.

- **Strong confirm**: `llvm.md` (`ConstantPointerNull::get(PointerType*)` is
  the gold-standard precedent; per-pointer-type interned).
- **Diverge**: `charon.md` (outer `ty` + inner `PtrNoProvenance(0)`
  — fuses; stage-4 plan equivalent), `mrustc.md` (implicit:
  zero-bytes + no-reloc + context type — fails vtable-vacant-slot
  canonicalisation), `compcert.md` (`Vnullptr` is untyped — same
  failure mode), `mlir.md` (no `NullAttr`, materialised per-type
  via `llvm.mlir.zero` op), `regseq-model-old.md` (absent).

LLVM is the only project that solves typed-null cleanly at the
const layer. Stage-4 plan stands. Rationale: vtable vacant-slot
nulls (one per `(PrincipalTraitKey, vtable_entry_index)`) need
to be distinguishable in the canonical key from a null
`*const u8` backing a `&str` fallback path.

### 1.3 Lossless uninit at byte level — two precedents, both load-bearing

- **Lossless**: `charon.md` (`Byte::{Uninit, Value(u8),
  Provenance(Provenance, u8)}` — three-state per byte inside
  `RawMemory(Vec<Byte>)`; MiniRust-derived), `rustc.md`
  (`InitMask` lazy bit-mask: `Lazy{state}` for all-init / all-
  uninit, `Materialized` otherwise).
- **Lossy (collapse-to-zero)**: `mrustc.md`
  (`allocation_to_encoded` line 3089 explicit NOTE),
  `compcert.md` (`Init_space n` becomes zero bytes at
  `Globalenvs.v:910-924`).
- **Different granularity (value-level only, not byte-level)**:
  `llvm.md` (`UndefValue` / `PoisonValue` per-Type singletons —
  doesn't address per-byte padding), `mlir.md` (`llvm.mlir.undef`
  / `llvm.mlir.poison` ops — same).
- **Absent**: `regseq-model-old.md`.

Stage-4 requires lossless (spec.md §9.7 line 1614-1616:
distinguish "uninit" from any byte pattern). charon's `Byte`
three-state is the design template; rustc's `InitMask` is the
storage-efficient precedent (lazy bitmask) — adoption decision
is per §2.3.

## 2. Real forks (stage-4 must decide deliberately)

### 2.1 Inline-only vs hybrid scalar type-tag (3-3 split)

- **Inline-only** (spec.md §9.7 plan): scalars carry their type
  inside the variant; aggregates resolve element types via the
  child `ConstId`'s `Const`.
- **Hybrid (inline + outer wrapper)**: charon, regseq-model-old,
  compcert init_data.

Trade-off:
- Hybrid eases per-context type queries — `get_ty()` is O(1)
  without interner resolution; useful for renderers and
  diagnostic paths that don't already have the `Ty` in scope.
- Inline-only means scalars are self-contained; aggregate /
  relocation / null variants need explicit `TyId` fields anyway
  (`NullPtr(TyId)`, `Aggregate(TyId, Vec<ConstId>)`?). The
  outer-wrapper question collapses into "do non-scalar variants
  carry a redundant outer `ty` field, or does each variant carry
  exactly the type it needs?"

D-4-1 must decide explicitly with worked-example pressure
(vtable instance values, generic-const args, struct-aggregate
literals).

### 2.2 Aggregate as first-class variant vs demote to synthetic static

- **First-class variant** (spec.md §9.7 plan, stage-4-plan
  `Aggregate(Vec<ConstId>)`): aggregates live inside `Const`.
  MLIR's `ArrayAttr<Vec<Attribute>>` and rustc's
  `ValTreeKind::Branch(Box<[Const]>)` are precedent.
- **Demote-to-static**: aggregates do NOT have a `Const`
  variant; instead the bridge synthesises an immutable Static
  Item per aggregate value, and references to that aggregate go
  through `InstanceKind::Static` (already in stage-3 vocabulary
  via vtable instances). Three projects take this path:
  `mrustc.md` (eager-promotion to fresh synthetic statics),
  `compcert.md` (aggregates exist only in `init_data` lists for
  globals, not in `val` or `Cminor.constant`), `regseq-model-old.md`
  (only `Rvalue::Aggregate` at runtime — no aggregate-as-const
  at all).

This was NOT in the original D-4-1 framing. The demote path
removes one variant from `Const` (no aggregate) at the cost of
introducing a synthesis path for every aggregate-shaped const.
Asymmetry: stage-3 already synthesises Static Items for vtable
instances (§13.2), so the demote path has partial precedent in
the IR. But it punts content-uniqueness from `ConstId` to
`InstanceKey` for aggregates, complicating the F4-2 story.

D-4-1 must address explicitly. See §6 for the worked-example
that disambiguates.

### 2.3 Three-state Byte vs InitMask

Both are lossless. Different storage models:
- **`Byte` three-state per element** (charon): direct,
  cache-locality friendly for small allocations; expensive for
  large all-init or all-uninit ranges.
- **`InitMask` lazy bitmask** (rustc): O(1) for all-init /
  all-uninit; materialised only when needed.

Stage-4 lean toward charon's `Byte` (smaller worked examples,
direct match for vtable instance value bytes; rustc's
`InitMask` is more pertinent at MIR-allocation scale that we
don't carry forward into RegseqModel).

### 2.4 Phase split necessity — justification rests on F4-2 alone

**0-of-5 non-rustc projects validate stage-4's two-phase
const-validation plan.**

- `charon.md`: no const interner; sidesteps the cycle.
- `mrustc.md`: no interner; sidesteps.
- `mlir.md`: const-in-attribute (uniquer per `MLIRContext`); no
  phase split.
- `compcert.md`: theorem-level validity; no runtime phase
  ordering.
- `regseq-model-old.md`: no const-uniqueness invariant at all.

The justification for stage-4's phase split (type-level before
`TyInterner`; value-level after) must therefore rest on F4-2
fingerprint determinism + the spec.md §3.3 cycle (`GenericArg::
Const(ConstId)` participates in nominal `Ty` identity), not on
prior-art necessity. Discussions.md will record this explicitly
when D-4-1's adjacent decisions land.

## 3. Genuinely novel territory (no prior art)

### 3.1 Cross-build content-addressed const identity via stable-identity tuple

5-of-5 non-rustc projects: **negative**.

- charon: relocations are local `(FunDeclId, BoxedArgs)`.
- mrustc: relocations key by `HIR::Path` verbatim.
- MLIR: per-`MLIRContext` storage uniquer; bytecode is
  allocation-order-dependent.
- LLVM: per-`LLVMContext` interner; in-memory pointers; bitcode
  is the stable form (`negative result`).
- CompCert: `ident := positive`, build-local.

Only adjacent precedent: rustc's `HashStable for AllocId` at
`compiler/rustc_middle/src/ty/impls_ty.rs:67-76` resolves the
local `AllocId` to its `GlobalAlloc` target before hashing,
deliberately discarding the local handle. Spec.md §9.7's
4-tuple `(regseq_instance_kind_rank, item_def_path_hash,
canonical_generic_args, fn_shim_disambig_or_None)` is
operationally equivalent.

This means: stage-4's cross-build encoding inherits the
technique from rustc but is novel relative to every IR-level
project that ingests rustc state. The 4-tuple is the single
load-bearing invention of the stage.

## 4. Patterns to deliberately avoid

### 4.1 Defensive fallbacks in const-eval (`regseq-model-old.md`)

`crates/regseq-model-old/src/extract/walk.rs:707` `fallback_literal`
silently substitutes zero when const-eval fails. Direct
violation of the "no defensive fallbacks" methodology rule. The
stage-4 bridge must reject (raise `BridgeError`), not paper over.

### 4.2 Build-local IDs as cross-build identity (multiple)

- `regseq-model-old.md`: `ConstantId` derived from rustc `DefId`
  allocation order — not stable across compiles.
- `mrustc.md`: relocations key by `HIR::Path` verbatim (local).
- `charon.md`: `FunDeclId` / `GlobalDeclId` from atomic counter.

Stage-4's 4-tuple must NOT contain any local `InstanceId` or
`ItemId` — the encoding is purely stable identity.

### 4.3 Stratified universes for const flavours (rustc / mrustc / CompCert)

Three projects layer the const types by use-context:
- rustc: `ty::Const` (pre-codegen, valtree-shaped) +
  `mir::Const` (MIR operand) + `mir::ConstValue` (post-eval
  bytes-with-relocations).
- mrustc: `MIR::Constant` (pre-eval, path-bearing) +
  `EncodedLiteral` (post-eval, bytes).
- CompCert: 4 universes (`val` runtime / `init_data` static-init
  / `Cminor.constant` IR-operand / `Csyntax.Eval` source).

The stratifications all exist because of pre-eval / post-eval
splits or layering across IR levels. Stage-4's single-layer
choice (one `Const` enum) is justified by the post-mono
guarantee: rustc has already evaluated everything reaching us;
no pre-eval shape needs to be carried. This is the load-bearing
benefit of the Q3/Q5/Q6 architectural decisions as applied to
constants. Document explicitly.

### 4.4 Loss of distinction at lowering (mrustc / CompCert)

- mrustc's `allocation_to_encoded` collapses uninit-as-zero
  (line 3089).
- CompCert's `Init_space` becomes zero bytes
  (`Globalenvs.v:910-924`).
- mrustc's `scalar_to_literal` silently narrows `U128` / `I128`
  inconsistently (`walk.rs:683` vs `mir_mapping.rs:343`).

Stage-4 must preserve every distinction rustc gives us at IR
construction. No silent narrowing, no zero-collapse. If a
distinction is genuinely irrelevant for downstream codegen, it
gets dropped at the codegen layer, not at IR construction.

## 5. New ideas surfaced (worth weighing)

### 5.1 Splat optimisation (MLIR)

`DenseElementsAttr` stores a single value when all aggregate
elements are equal. Could compress vtable vacant-slot
populations (typically multiple `NullPtr(canonical_vacant_sig_id)`
in the same vtable). Low-cost adoption candidate; defer to
implementation-time decision unless it changes the spec shape.

### 5.2 Float natural-width storage (mrustc cautionary)

mrustc collapses to `double` — lossy for `f16` / `f128`. Spec.md
§10 admits `F16` / `F32` / `F64` / `F128`. Stage-4 carries
`Float(FloatKind, u128 bits)` per spec — the `u128` width is
sufficient for `F128`, but be explicit that smaller floats
zero-pad to fill. Treat as confirmed by mrustc's negative.

### 5.3 Demote-to-static aggregate alternative

See §2.2. Genuinely new fork in the road, surfaced by 3 projects
taking the demote path.

## 6. Implications for D-4-1

The prior-art evidence supports the bulk of the original D-4-1
framing (Option A: flat enum with per-flavour variants), with
**three material adjustments and one new fork**:

1. **Adopt charon's three-state Byte for uninit** (§1.3, §2.3).
   Replace generic `Uninit { size }` with byte-level
   distinguishability. Worked example: vtable instance values
   carrying both function-pointer relocations and uninit
   padding bytes between fields.

2. **Decide explicitly on inline-only vs hybrid type-tag**
   (§2.1). Currently underspecified — spec.md §9.7 implies
   inline, but non-scalar variants will need explicit `TyId`
   fields, raising the "redundant outer `ty`" question.

3. **Confirm typed-null** (§1.2) — LLVM-strong; stage-4 plan
   stands. No revision needed.

4. **NEW: Decide aggregate-as-variant vs demote-to-static**
   (§2.2). Three projects take demote; stage-4 plan goes
   first-class. Asymmetry deserves explicit framing — partial
   precedent in stage-3's vtable Static synthesis suggests
   demote is feasible, but content-uniqueness moves from
   `ConstId` to `InstanceKey`, complicating F4-2.

5. **Confirm cross-build 4-tuple stable encoding** (§3.1) —
   genuinely novel; rustc's `HashStable for AllocId` is the
   operational equivalent precedent. No revision needed.

6. **Justify phase split via F4-2 alone, not prior-art** (§2.4)
   — record explicitly in `discussions.md` when D-4-1 lands.

Sub-decisions D-4-1.1 through D-4-1.7 from the original framing
remain valid; expect to add D-4-1.8 (uninit shape: `Byte`
three-state vs `InitMask`-style bitmask) and D-4-1.9
(aggregate-vs-demote-to-static).

## 7. Per-project file index

Sorted by direct relevance to D-4-1.

| File                       | Size  | Role                                                                                                                                                  |
| -------------------------- | ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `rustc.md`                 | 66 KB | Source-of-truth. `ty::Const` / `ConstKind` / valtree + `mir::Const` / `ConstValue` / `Allocation`. The `HashStable for AllocId` precedent.            |
| `charon.md`                | 31 KB | Closest peer. 13 `ConstantExprKind` variants → 2 post-simplify. The `Byte` three-state for uninit. Local-ID relocations (negative for cross-build).   |
| `llvm.md`                  | 52 KB | Gold-standard hierarchy. `ConstantData` / `ConstantAggregate` / `ConstantExpr` / `ConstantPointerNull` / `UndefValue` / `PoisonValue`. `GlobalValue`-as-Constant relocations. |
| `mlir.md`                  | 37 KB | Attribute-not-value model. Uniqued attributes; splat optimisation; closed-vocabulary divergence.                                                       |
| `mrustc.md`                | 32 KB | Pre-stable-const-eval reference. Two-layer (`MIR::Constant` + `EncodedLiteral`); demote-to-static; defensive-fallback anti-pattern.                   |
| `compcert.md`              | 33 KB | Formal-verification pressure. 4-universe stratification; no aggregate at value level; theorem-level validity.                                          |
| `regseq-model-old.md`      | 19 KB | In-tree reference. 11 scalar-only `LiteralValue` variants; explicit anti-patterns flagged. Re-derivation analysis: 7-of-8 don't survive.               |

---

**Update workflow.** When D-4-1 lands, this README does NOT get
revised — it's the snapshot of evidence at the framing moment.
If a stage-4 sub-decision (D-4-2+) needs additional prior-art
evidence (e.g., comparing string-literal handling across LLVM
`ConstantDataArray` vs charon `Literal::Str`), append a new
section here OR a sibling file `<topic>.md` in this folder. The
per-project files are frozen artefacts at first-launch state.
