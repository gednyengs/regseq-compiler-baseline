# Stage-4 Prior-Art: regseq-model-old (in-tree reference)

Stage-4 prior-art research on `crates/regseq-model-old/`, the
previous RegseqModel iteration retained in-tree as reference.

**Methodology framing (binding).** Per `../../README.md` §2:
- "Re-derive from evidence; do not import baggage."
- "Treat the `*-old` set as reference per the 're-derive from
  evidence' rule -- consult deliberately, do not lift wholesale."

This file's job is **not** "what did the old model do, let's
adopt it." It is "what did the old model do, what worked, what
didn't, and what survives re-derivation under the new framing
(post-mono `optimized_mir`, `mir_opt_level=0`, the curated
cleanup-pass whitelist)?"

---

## Source map

| Path                                                                  | Role                                       |
| --------------------------------------------------------------------- | ------------------------------------------ |
| `crates/regseq-model-old/src/ir/operand.rs`                           | `Operand` enum + `LiteralValue` enum (the const-value type) |
| `crates/regseq-model-old/src/ir/rvalue.rs`                            | `Rvalue::Aggregate` (where const aggregates would go but don't) |
| `crates/regseq-model-old/src/model/constant.rs`                       | Named `Constant` item carrier              |
| `crates/regseq-model-old/src/identity/decl_id.rs`                     | `ConstantId` newtype (built on `DeclId`)   |
| `crates/regseq-model-old/src/verify/rules/body.rs`                    | Const reference resolution + literal type inference (`literal_type`, `operand_type`) |
| `crates/regseq-model-old/src/verify/phase_2.rs`                       | Two-phase validation entry point (no const-specific phase) |
| `crates/regseq-driver-old/src/construction.rs`                        | Bridge `BuildContext.constants` map (DefId -> ConstantId) |
| `crates/regseq-driver-old/src/construction/walk.rs`                   | `build_constant`, `eval_const`, `fallback_literal` |
| `crates/regseq-driver-old/src/construction/mir_mapping.rs`            | `map_const`, `map_operand`, `scalar_to_literal` (operand-side, the strict-Option variant) |

No top-level `README.md` exists for `regseq-model-old`. No
explicit `#[deprecated]` markers; the crate is "reference,
not deprecated" per the project methodology rule above.

---

## Findings

### 1. Const-value type

The const-value carrier is **`LiteralValue`**, an
11-variant enum at `ir/operand.rs:37-60`:

```rust
pub enum LiteralValue {
    U8(u8), U16(u16), U32(u32), U64(u64), Usize(u64),
    I8(i8), I16(i16), I32(i32), I64(i64), Isize(i64),
    Bool(bool),
}
```

The accompanying named-item carrier is **`Constant`**, a
4-field struct at `model/constant.rs:19-29`:

```rust
pub struct Constant {
    pub id: ConstantId,
    pub name: Symbol,
    pub ty: RegseqType,
    pub value: LiteralValue,
}
```

**Coverage gaps relative to stage-4's required surface
(spec.md §9.7):**
- No `Char` variant (Unicode scalar).
- No `Float` variant (`f16`/`f32`/`f64`/`f128` -- floats not modelled at all).
- No `U128`/`I128` (`map_const`'s `scalar_to_literal` returns `None` at `mir_mapping.rs:353,361`; `walk.rs`'s `scalar_to_literal` silently narrows to 64-bit at `walk.rs:692,700`).
- No ZST variant.
- **No aggregate variant** (see Finding 3).
- **No relocation variant** (see Finding 4).
- **No null-pointer variant** (see Finding 5).
- **No uninit/padding variant** (see Finding 6).

The `Constant`-vs-`LiteralValue` split mirrors the rustc
"named static/const item" vs "inline literal" distinction;
the named-item path goes through `Operand::Constant(ConstantId)`
(`operand.rs:22`) whereas inline literals go through
`Operand::Literal(LiteralValue)` (`operand.rs:24`).

### 2. Type-tag placement

**Hybrid externalised + inline** -- the same shape charon
exhibits:

- **Inline** at `LiteralValue`'s variant tag: `U32(_)` carries the
  width; `literal_type` at `verify/rules/body.rs:682-694`
  recovers `RegseqType::Scalar(ScalarType::U32)` purely from
  the variant.
- **Externalised** at the wrapper: `Constant.ty: RegseqType`
  carries the type (`model/constant.rs:26`), and
  `Operand::Constant(ConstantId)` resolves to that ty via
  `verify/rules/body.rs:677` (`constants.get(&cid.decl_id()).map(|c| c.ty.clone())`).

**Stage-4 evidence weight.** Confirms the hybrid pattern is
viable -- but `LiteralValue` covers only inline scalars. The
old model never had to answer "what does the wrapper-level
`ty` look like for a const that contains a relocation or a
typed null?" -- it didn't model those at all.

### 3. Aggregate constants

**NOT REPRESENTED at the const layer.** Aggregates appear only
as `Rvalue::Aggregate(AggregateKind, Vec<Operand>)` at
`ir/rvalue.rs:42`:

```rust
pub enum AggregateKind {
    Array(RegseqType),
    Tuple,
    Adt { type_id: TypeDefId, variant_index: u32 },
}
```

This is a *runtime-construction* shape: the children are
`Vec<Operand>` (each operand is a Place, Move, Constant ref, or
Literal), and the whole rvalue executes as a SetUp at runtime.

**Implication.** A source-level const aggregate
(e.g. `const FOO: [u32; 3] = [1, 2, 3];`) cannot be expressed
as a single `Constant` value -- the `Constant.value:
LiteralValue` field admits scalars only. `eval_const` at
`walk.rs:675-681` calls `try_to_scalar()` on the const-eval
result and returns `None` for non-scalar shapes; combined with
the `fallback_literal` substitution (Finding 7 / §9), the old
bridge would silently emit `LiteralValue::U32(0)` for
non-scalar constants -- a known scope reduction.

**Re-derivation note.** Stage 4 *must* model aggregates as
first-class const values per §9.7's coverage list (vtable
instances, string literals backing `&str`, struct/array/enum
constants). The old model's `Rvalue::Aggregate`-only path is
not a survivable design.

### 4. Relocation handling

**Not represented.** No function-pointer-as-constant, no
static-as-constant, no vtable-pointer-as-constant.
`Operand::Constant(ConstantId)` is a *symbolic reference* to a
named `Constant` declaration, not a relocation in the §9.7
sense. The old model had no facility for "this constant's
value is the address of that other instance."

A consequence: vtable instance values (§13.2) -- aggregate
constants whose fields are function pointers to vtable shim
Function Items -- could not be expressed in the old model
even in principle. The old codebase also had no vtable
synthesis (`Ty::Dynamic` was outside its scope).

### 5. Null pointer typing

**Not represented.** No null-pointer variant of `LiteralValue`,
no null-bearing variant of `Operand`. The old model handled
no pointer types at the const layer at all.

### 6. Uninit / padding

**Not represented.** No uninit variant, no init-mask analog,
no per-byte init tracking. The old model never crossed the
boundary into byte-buffer-shaped consts.

### 7. Validation phase

**No const-specific validation phase.** The verifier runs in
two phases (`verify/phase_1.rs`, `verify/phase_2.rs`), but
neither contains const-content checks. Const validation is
limited to **reference resolution** at body verification time
(`verify/rules/body.rs:331,677`):

- `Operand::Constant(cid)` must resolve to a declared
  `ConstantId` in `model.constants` (lookup table built at
  `body.rs:97-102`).
- Operand types must match expected types
  (`operand_types_match` at `body.rs:410`).

**No content-uniqueness invariant.** Two source-level
constants with identical types and values get distinct
`ConstantId`s -- determined by allocation order at
`construction.rs:735-737` (DefId-keyed). The model has no
content-keyed interner.

**Phase split.** The verifier's phase-1/phase-2 split
(`verify/phase_2.rs:1-21`) is about driver-binding population
(spec.md Section 13's "binding slot is None" rule), not about
type-level vs value-level consts. The cycle that motivates
stage-4's two-phase const validation does not arise because
the old model has no const interner -- the same sidestep
charon takes.

### 8. Cross-build determinism

**No deterministic-hashing strategy.** `ConstantId` derives
from `DeclId` allocated by
`construction.rs::BuildContext::alloc_and_record(def_id)` --
i.e., the order in which the bridge encounters rustc `DefId`s
during HIR/MIR walking. Order depends on rustc internals (the
HIR walk schedule, the MIR collection sequence) and is not
guaranteed stable across compiles or rustc versions.

No `DefPathHash` analog, no `StableHash` impl, no canonical
const-eval result form. Determinism is a non-goal in the
existing code.

---

## 9. Re-derivation analysis

For each finding, whether the old design was made under
pre-monomorphisation assumptions that no longer apply, and
whether the choice survives re-derivation under stage-4
framing.

| Finding   | Pre-mono assumption?                                                            | Survives re-derivation?                                                                                       |
| --------- | ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| 1 (variants) | Yes -- the bridge tried `try_to_scalar()` and fell back on failure (`walk.rs:677`). Pre-mono code paths assumed const-eval might fail or remain symbolic, so the model defaulted to a narrow scalar surface and a defensive zero. | **No** for the 11-variant scalar-only set. Post-mono per Q3/Q5/Q6 of architecture.md and the no-defensive-fallbacks methodology rule (`README.md` §2): all consts that survive monomorphisation MUST have a fully-evaluated representation, and the variant set must cover the §9.7 universe. |
| 2 (tag placement) | No, this is a structural decision. | **Partially.** The hybrid (inline + externalised) pattern is acceptable for scalars but the wrapper-level `ty` field's role at higher arities (typed null, relocation, aggregate) was never tested by the old model. Stage 4 must decide deliberately, not inherit by default. |
| 3 (aggregates as Rvalue) | Yes, profoundly. The old model never assumed const aggregates would arrive pre-evaluated; it deferred to runtime construction (`Rvalue::Aggregate`). | **No.** Post-mono, const aggregates ARE pre-evaluated (rustc's `mir::ConstValue::Indirect` / valtree branches). Stage 4's `Const::Aggregate(Vec<ConstId>)` is the right shape; the runtime-construction split would mis-classify static initialisers and vtable instance values. |
| 4 (relocations) | The old model had no vtable synthesis, no `dyn` translation, no static initialisers needing function-pointer relocations -- so no relocation surface was needed. | **No** -- stage-4 inherits Q3-onwards' larger ambit (vtable synthesis, statics with init expressions). Relocations are a hard requirement (vacant vtable slots in §13.2 alone). |
| 5 (null typing) | Same as 4 -- no pointer-bearing const surface in scope. | **No.** Vacant vtable slots (§13.2) are typed null pointers, mandatory in v1. |
| 6 (no uninit) | Same as 4. | **No.** rustc's `Allocation::InitMask` can carry uninit bytes through aggregate consts; padding-byte distinguishability is a stage-4 invariant per `memory.md` Section 2. |
| 7 (no const phase) | Yes -- the absence of an interner sidestepped the cycle. The old model relied on `ConstantId` (a `DeclId`-based local handle) for identity, which never needed cross-table validation against types or instances. | **No** as a precedent for "we don't need a phase split." Stage 4 introduces an interner (per spec.md §9.7 invariant 11) for content uniqueness in service of F4-2 fingerprinting; once the interner exists, the type-level vs value-level cycle is real. The old model's no-interner choice doesn't survive because F4-2 determinism is a stage-4 hard requirement that the old model never had. |
| 8 (no determinism) | Determinism wasn't a stated old-model goal. | **No.** F4 / F4-2 (spec.md §16) require cross-build identity. The old model's `DefId`-allocation-order ConstantIds would re-shuffle on rustc internal changes. Stage 4 needs `DefPathHash`-derived stable identity for relocation targets. |

**Net.** Of the eight findings, seven do not survive
re-derivation. The only partial survivor is the hybrid
type-tag placement (Finding 2), and even that needs explicit
re-decision because the old model never exercised the cases
where the wrapper-level `ty` is load-bearing.

---

## 10. Specific anti-patterns to avoid

These are concrete code-shape lessons from the old codebase that
stage 4 should specifically NOT replicate:

1. **`fallback_literal` (silent zero substitution).** At
   `walk.rs:666,707-716`, when `eval_const` fails or
   `ty_to_regseq` cannot translate the type, the bridge
   inserts a zero of the requested type:
   ```rust
   let value = eval_const(cx.tcx, def_id).unwrap_or_else(|| fallback_literal(&ty));
   ```
   This is exactly the kind of defensive fallback the new
   methodology forbids (`README.md` §2 "No defensive
   fallbacks"). Stage 4 must reject the input instead.

2. **Two divergent `scalar_to_literal` functions.** One in
   `construction/mir_mapping.rs:343-366` returns
   `Option<LiteralValue>` (proper failure path: `None` for
   `U128`/`I128` and unsupported types). Another in
   `construction/walk.rs:683-705` silently narrows
   `U128`/`I128` to 64-bit (`walk.rs:692,700`) and returns
   `LiteralValue::U32(bits as u32)` for any unrecognised type
   (`walk.rs:703`). Inconsistent error handling at the bridge
   layer. Stage 4 should expose a single canonical translator
   with strict-Option semantics.

3. **`ConstantId` from `alloc_and_record(def_id)`.**
   `construction.rs:737` keys constants by rustc DefId encounter
   order. Two unrelated rustc invocations may schedule HIR/MIR
   walk differently, producing different `ConstantId`
   assignments. Stage 4 needs `DefPathHash`-derived identity
   (per `memory.md` Section 2's "Cross-build canonicalisation
   uses stable identity" principle).

4. **`u128`/`i128` silent narrowing.** Even where the old
   model attempts to be permissive (`walk.rs:692`), the
   narrowing is undetectable by downstream consumers. Stage 4
   should either model `u128`/`i128` natively or reject them
   with `BridgeError`.

5. **Two-phase verifier without const phase.** The phase
   structure exists (`verify/phase_2.rs`), so naming
   collisions are a risk -- stage 4's "type-level vs
   value-level" const phase split must be clearly distinct
   from the existing "phase-1 declaration / phase-2 binding"
   structure inherited from the old codebase. Use distinct
   terminology (e.g., "const-phase A" / "const-phase B" or
   the `TypeConstInterner` / `ConstInterner` two-table
   shape).

---

## 11. Stage-4 applicability summary

What the old model gives stage 4:

| Question                  | Old-model evidence                                                                       | Stage-4 disposition                                                                                         |
| ------------------------- | ---------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| Inline scalar tag         | Confirms inline tag works for the scalar subset.                                         | Adopt (already pinned in spec.md §9.7).                                                                     |
| Wrapper-level `ty`        | Hybrid present at `Constant.ty`, but only exercised for scalar `value`s.                 | Re-decide deliberately under the larger variant set; old evidence is too weak to settle the question.       |
| Per-flavour vs flat enum  | Old model is a flat per-flavour enum (each width separate).                              | Convergent with stage-4's Option A. Old model's narrowness leaves no aggregate / null / reloc choice for us to learn from. |
| Aggregates                | NOT REPRESENTED (runtime-construction-only).                                             | Stage 4 must add as first-class. No prior-art guidance from old model.                                      |
| Relocations               | NOT REPRESENTED.                                                                         | No prior-art guidance.                                                                                      |
| Null pointers             | NOT REPRESENTED.                                                                         | No prior-art guidance.                                                                                      |
| Uninit                    | NOT REPRESENTED.                                                                         | No prior-art guidance.                                                                                      |
| Phase split               | Verifier two-phased on declarations/bindings, NOT on consts.                             | Distinct concept; pick distinct terminology to avoid name collision.                                        |
| Determinism               | DefId-allocation-order, fragile.                                                         | Stage 4's `DefPathHash`-keyed shape is novel territory (no prior-art validation here).                      |

The old model's primary contribution is **negative
evidence**: it shows what a const-vocabulary looks like when
designed pre-mono and bounded by a narrow driver-DSL surface.
Almost every choice that worked in the old context fails to
survive the new framing -- which itself is informative for
what stage 4 must explicitly redesign.

---

## 12. Open questions

1. **`Constant` named-item carrier shape.** Stage-4 spec.md
   §9.4's `InstanceKind::Const { value: ConstId }` and
   `InstanceKind::Static { init: Option<ConstId>, .. }`
   replace the old `Constant { id, name, ty, value:
   LiteralValue }` struct. Question: does stage 4 retain a
   *named*-`Constant` analog (carrying `name: StringId`,
   `ty: TyId`, `value: ConstId`), or is the const-item-as-
   Instance shape (per the new IR's Item-and-Instance
   convention) sufficient? The old model's `Constant` and
   `Function` siblings were both per-decl items, mirroring
   rustc's DefId universe; the new IR generalises both to
   `Item` + `Instance`. Confirm during stage-4 work that no
   Constant-item-specific carrier is needed.

2. **`Operand::Constant(ConstantId)` analog at MIR-bodies
   stage.** The old `Operand::Constant(ConstantId)` lives in
   the body IR, not the const IR. Stage 11 (MIR bodies) will
   define operand shapes; stage 4 should ensure its `ConstId`
   shape supports both the inline-literal-equivalent path
   (post-mono evaluated value embedded in the operand) and
   the named-item-reference path (operand refers to a static
   or const item by `InstanceId`).

3. **`SymbolInterner` -> `StringInterner` migration.** The
   old `Constant.name: Symbol` is interned via
   `SymbolInterner`. Stage 1's new `StringInterner` replaces
   it. Already orthogonal to const-vocabulary work but flag
   in case stage 4 spec touches naming.

---

## Final note

The `*-old` reference rule (`README.md` §2) means this file
is consulted, not lifted. Stage 4's `Const` enum is not
"`LiteralValue` plus more variants"; it is a re-derived
design grounded in spec.md §9.7's universe coverage,
stage-3's decisions on layout-resolved types, and the
no-defensive-fallbacks + cross-build-determinism methodology
rules. The old model's narrow scalar-only surface confirms
that the inline-tag path works for scalars and that anything
beyond requires fresh design -- which is the work stage 4 is
opening.
