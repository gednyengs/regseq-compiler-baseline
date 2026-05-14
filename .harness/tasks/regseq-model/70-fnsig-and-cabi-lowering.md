# regseq-model-70: `FnSig` + `FnSigInterner` + `CAbiLowering` (Predicate Outcomes Stored)

## Description

Land the function-signature canonical schema from spec.md §14
(spec.md:5221-5841): the `FnSig` struct (spec.md:5256-5267 -- the
canonicalisation surface with `inputs` / `output` / `abi` /
`c_variadic` / `safety`), the `FnSigInterner` BiMap
(spec.md:5344-5358), the `CAbiShape` / `CAbiLowering` /
`SubstReason` types from §14.5 (spec.md:5394-5434 -- the C-ABI
admissibility predicate's **output shape**, NOT the predicate
itself), the FnSig canonical byte encoding for the `Ty::FnPtr(sig)`
canonicaliser tag-12 from §12.3 (spec.md:4327), and `FnAbi` re-export
visibility (the enum body lives in `ty.rs` per P1a-DS-02; this task
ensures `FnSig.abi: FnAbi` is well-typed).

The bridge-side recursive admissibility predicates
`CAbiPositionAdmissible(ty, position)` and `CFieldAdmissible(ty)`
(spec.md:5486-5550) are **bridge-axis territory** (P1a-BR-03b).
This task carries only the **outcome** -- the per-position
`CAbiShape` (Direct / Substituted) decision the bridge stores in
`sig_c_abi_lowerings[id]` per spec construction invariant 16.

## Input Context

- `.harness/SESSION.md`
- `.harness/spec-batches/regseqmodel-phase-1a.md` (this task is
  P1a-DS-06)
- `docs/discussions/regseq-model/spec.md` §14 (spec.md:5221-5841)
- `crates/regseq-model/src/ty.rs` (P1a-DS-02 -- `FnAbi`, `Safety`)
- `crates/regseq-model/src/ty_interner.rs` (P1a-DS-04 --
  `TyInterner` for the `inputs` / `output` `TyId`s)
- `crates/regseq-model/src/args_key.rs` (P1a-DS-04 -- the
  SHA-256 truncation pattern; the FnSig canonical bytes feed into
  the `Ty::FnPtr` row of the deep canonicaliser)
- `crates/regseq-model/src/model.rs` (P1a-DS-01 -- `RegseqModel`
  hosts `sigs: FnSigInterner` and `sig_c_abi_lowerings:
  IndexVec<FnSigId, Option<CAbiLowering>>`; this task replaces the
  `FnSigInterner` stub)
- `crates/regseq-model/src/ids.rs` (`FnSigId`, `TyId`)
- `.harness/tasks/regseq-model/65-root-regseq-model-and-foundation.md`
- `.harness/tasks/regseq-model/66-ty-enum-and-helpers.md`
- `.harness/tasks/regseq-model/68-ty-interner-and-args-key.md`

## Implementation Notes

Create new modules:

- `crates/regseq-model/src/fn_sig.rs` -- `FnSig`, the canonical byte
  encoding helper for `Ty::FnPtr` (the §12.3 tag-12 row).
- `crates/regseq-model/src/fn_sig_interner.rs` -- `FnSigInterner`
  BiMap, custom serde.
- `crates/regseq-model/src/cabi.rs` -- `CAbiShape`, `CAbiLowering`,
  `SubstReason`.

Re-export from `lib.rs`. Replace the `FnSigInterner` stub
introduced by P1a-DS-01 in `model.rs`.

### Public API (verbatim from spec)

#### `FnSig` (spec.md:5258-5274)

```rust
pub struct FnSig {
    /// Positional argument types. `inputs[i]` MUST NOT be `Ty::Never`
    /// (spec.md:5278-5285 -- bridge rejects with `BridgeError::UnsupportedNeverInput`).
    pub inputs:     Vec<TyId>,

    /// Return type. MAY be `Ty::Never` (divergent functions are
    /// well-formed at the IR level) and MAY be `Ty::Tuple { fields: [] }`
    /// (Rust unit `()`; the standard no-value return shape at FnAbi::C
    /// boundaries per §14.5).
    pub output:     TyId,

    /// `Rust` or `C` (load-bearing for type identity per Q2; embedded
    /// as a structural field so two FnSigs differing only in ABI are
    /// distinct in the interner).
    pub abi:        FnAbi,

    /// Trailing `...` (C variadic). Distinct types in rustc; load-
    /// bearing for type identity. `c_variadic == true` implies
    /// `abi == FnAbi::C` AND `inputs.len() >= 1` (validator-enforced
    /// at P1a-VAL-03).
    pub c_variadic: bool,

    /// `unsafe fn` marker; distinct types in rustc.
    pub safety:     Safety,
}
```

The 5-field schema is the **canonicalisation surface** per
spec.md:5256-5274. Diagnostic-only data (parameter names,
declaration spans) is **not** carried on `FnSig`; if needed,
sibling tables keyed by `FnSigId` per spec.md:5268-5274.

`FnSig` derives `Clone, Debug, PartialEq, Eq, Hash, Serialize,
Deserialize`. The `Hash + Eq` implementation MUST cover exactly
the 5 schema fields (no diagnostic / sibling state); two FnSigs
with identical canonical schemas collapse to one `FnSigId` per
spec construction invariant 11.

#### `FnSigInterner` (spec.md:5345-5358)

```rust
pub struct FnSigInterner {
    storage: IndexVec<FnSigId, FnSig>,
    lookup:  HashMap<FnSig, FnSigId>,        // Hash + Eq over the §14.3 fields exactly
}

impl FnSigInterner {
    pub fn new() -> Self;
    pub fn intern(&mut self, sig: FnSig) -> FnSigId;
    pub fn resolve(&self, id: FnSigId) -> &FnSig;
    pub fn find(&self, sig: &FnSig) -> Option<FnSigId>;
    pub fn len(&self) -> usize;
    pub fn iter(&self) -> impl Iterator<Item = (FnSigId, &FnSig)>;
}
```

Same BiMap pattern as `TyInterner` (P1a-DS-04) and `ConstInterner`
(P1a-DS-05). Custom serde mirrors the same shape: on-wire is
`storage` only; `lookup` reconstructed at deserialise by direct
insertion preserving original `FnSigId` values.

#### `CAbiShape` / `CAbiLowering` / `SubstReason` (spec.md:5395-5434)

```rust
pub enum CAbiShape {
    /// Use the position's own `TyId` in its natural C representation.
    Direct,

    /// Substitute a different `TyId` at the C boundary.
    Substituted {
        emit_as: TyId,
        reason:  SubstReason,
    },
}

pub enum SubstReason {
    /// One or more repr(transparent) unwraps.
    TransparentChain,

    /// Fieldless repr(int) enum (every variant unit), no transparent wrapping.
    TagScalar,

    /// Composition: chain ultimately resolves to a tag scalar.
    TransparentChainAndTagScalar,
}

pub struct CAbiLowering {
    pub inputs: Vec<CAbiShape>,    // one per FnSig.inputs entry
    pub output: CAbiShape,
}
```

`CAbiShape`, `CAbiLowering`, `SubstReason` derive `Clone, Debug,
PartialEq, Eq, Hash, Serialize, Deserialize`.

The `sig_c_abi_lowerings: IndexVec<FnSigId, Option<CAbiLowering>>`
side table on `RegseqModel` (introduced by P1a-DS-01) is `Some(_)`
iff `FnSig.abi == FnAbi::C` (spec construction invariant 16,
spec.md:528-543); `None` otherwise. Length parity with `sigs` is a
construction invariant.

### `FnSig` canonical byte encoding (spec.md:4327)

The §12.3 deep canonicaliser tag-12 row (`Ty::FnPtr`) emits:

```text
12 = FnPtr followed by canonical FnSig content (§14.3)
```

The "canonical FnSig content" expansion is owned by this task. The
emitted bytes are:

```text
canonical_fn_sig:
  u32 LE input_count
  for each input: canonical_args(TyId) per §12.3 walk rules
  canonical_args(output)
  u8 abi_disc            // FnAbi::Rust = 0, FnAbi::C = 1
  u8 c_variadic          // 0x00 = false, 0x01 = true
  u8 safety_disc         // Safety::Safe = 0, Safety::Unsafe = 1
```

(The exact tag layout is not pinned in spec.md but the schema is:
"5 fields exactly per spec.md:5260-5267 with multi-byte integers LE
per the §12.3 global rules". This task's authoritative
implementation is the `canonical_fn_sig(sig: &FnSig, model:
&RegseqModel) -> Vec<u8>` function; consumers (the §12.3 deep
canonicaliser; the `ArgsKey` of any `Ty::FnPtr` term) call it via
the `Ty::FnPtr` row.)

The `canonical_args(TyId)` recursion is provided by P1a-DS-04
(`crates/regseq-model/src/canonical.rs` or wherever DS-04 located
the `canonical_ty(TyId, &RegseqModel) -> Vec<u8>` helper). This
task implements `canonical_fn_sig` and provides the integration
point; DS-04's `canonical_ty` arm for tag-12 calls
`canonical_fn_sig`.

If P1a-DS-04 ships a `// FORWARD: P1a-DS-06` stub in its
`canonical_ty` tag-12 arm, this task removes the stub and inserts
the real call.

### `FnSig` construction invariants (documentation; validator-enforced later)

Per spec.md:5276-5302:

- **Never-input rejection**: `inputs[i]` MUST NOT be `Ty::Never`.
  Bridge rejects with `BridgeError::UnsupportedNeverInput`.
  Document on `inputs` field; enforcement at P1a-BR-03b / VAL-03.
- **Unit return**: `output` MAY be `Ty::Tuple { fields: [] }`.
- **`c_variadic == true`** implies `abi == FnAbi::C` AND
  `inputs.len() >= 1`. Document on `c_variadic` field; validator at
  P1a-VAL-03.

Per spec.md:5303-5332 the FnSig placement is **Instance-side** on
`InstanceKind::Function { fn_sig: FnSigId, body: Option<Body>, ...
}` per stage-12 D-12-1.4 (Option B). This task does not alter the
Instance shape (P1a-DS-10 owns it); it only ships the `FnSig` data
type and interner.

### `CAbiShape` invariants (documentation; validator-enforced later)

Per spec.md:5430-5475 the `CAbiLowering` shape pinning:

- **Type identity preservation**: `FnSig.inputs/output` preserve
  the **original Rust `TyId`s**; the lowering is *recorded
  alongside* in `sig_c_abi_lowerings`, not substituted into the
  FnSig.
- **`emit_as` composition**: substitution is "fully composed" --
  the predicate walks repr(transparent) + fieldless repr(int) enum
  to a fixed point and stores the final terminal `TyId`.
- **`CAbiLowering` is sibling-only metadata**: not part of FnSig
  identity; FnSigInterner's `Hash + Eq` excludes
  `sig_c_abi_lowerings` content (which it doesn't see anyway -- the
  side table lives on `RegseqModel`).

### Conventions

- **Closed-enum discipline**: `CAbiShape` and `SubstReason` are
  closed enums; no `#[non_exhaustive]`, no wildcard arms.
- **Doc comments on every public type / variant / function**.
- **No unicode in comments / docs**.
- **Derives**: `Clone, Debug, PartialEq, Eq, Hash, Serialize,
  Deserialize` for `FnSig`, `CAbiShape`, `CAbiLowering`,
  `SubstReason`.
- **`FnSigInterner` derives**: same custom serde shape as
  `TyInterner` and `ConstInterner` (storage-only on-wire).

## Non-Goals

- **No `CAbiPositionAdmissible` / `CFieldAdmissible` predicate
  bodies** -- P1a-BR-03b owns the recursive bridge-side checker
  with memoisation / cycle handling per spec.md:5486-5550. This
  task ships only the **output shape** (the `CAbiShape` /
  `CAbiLowering` types).
- **No `BridgeError::UnsupportedNeverInput` enforcement** --
  P1a-BR-03b at the bridge.
- **No bridge translation map** for ExternAbi -> FnAbi
  (spec.md:5236-5253) -- P1a-BR-03b.
- **No closure call-method `FnAbi::Rust` rewrite** --
  P1a-BR-04 / DS-08a.
- **No vacant-vtable canonical signature** -- P1a-DS-08b /
  P1a-BR-04 owns the `extern "C" fn(*mut ()) -> ()` shape.
- **No InstanceKind body shape** -- P1a-DS-10 owns the
  `InstanceKind::Function { fn_sig: FnSigId, ... }` slot.
- **No validator** -- P1a-VAL-02 / VAL-03 own:
  - Length parity `sig_c_abi_lowerings.len() == sigs.len()`
    (invariant 16 second clause).
  - Per-entry presence rule (`Some` iff `FnAbi::C`).
  - Arity check `lowering.inputs.len() == sigs[id].inputs.len()`.
  - Tag-scalar primitiveness (`Substituted.reason == TagScalar`
    requires `emit_as` to be `Ty::Int(_)` or `Ty::Uint(_)`).
  - `c_variadic == true` implies `abi == FnAbi::C` AND
    `inputs.len() >= 1`.
  - `Ty::Never` not in `inputs`.
- **No pretty-printer** -- P1a-PP-* axis (PP-02 covers
  `render_ty` for `Ty::FnPtr`).

## Test Plan

`unit-in-src` discipline. Tests live in
`crates/regseq-model/src/fn_sig.rs::tests`,
`crates/regseq-model/src/fn_sig_interner.rs::tests`, and
`crates/regseq-model/src/cabi.rs::tests`. Per `SESSION.md` Testing
Requirements, every public type / function gets at least one test.

### `FnSig` constructor tests

- Construct `FnSig { inputs: vec![], output: unit_ty, abi:
  FnAbi::Rust, c_variadic: false, safety: Safety::Safe }` (the
  empty `fn() -> ()` shape).
- Construct `FnSig { inputs: vec![u32_ty, u32_ty], output: u32_ty,
  abi: FnAbi::C, c_variadic: false, safety: Safety::Safe }`
  (`extern "C" fn(u32, u32) -> u32`).
- Construct `FnSig { inputs: vec![ptr_ty], output: never_ty, abi:
  FnAbi::Rust, c_variadic: false, safety: Safety::Safe }`
  (divergent function returning `!`).
- Construct `FnSig { inputs: vec![u32_ty], output: unit_ty, abi:
  FnAbi::C, c_variadic: true, safety: Safety::Safe }`
  (`extern "C" fn(u32, ...) -> ()` -- variadic).
- Construct `FnSig { inputs: vec![], output: u32_ty, abi:
  FnAbi::Rust, c_variadic: false, safety: Safety::Unsafe }`
  (`unsafe fn() -> u32`).

### `FnSig` equality / hash

- Two `FnSig`s with identical 5-field content are equal and hash
  equal.
- `FnSig { abi: FnAbi::Rust, .. }` is not equal to its sibling with
  `abi: FnAbi::C` (ABI is part of identity).
- `FnSig { c_variadic: false, .. }` is not equal to its sibling with
  `c_variadic: true`.
- `FnSig { safety: Safety::Safe, .. }` is not equal to its sibling
  with `safety: Safety::Unsafe`.
- `FnSig { inputs: vec![ty1, ty2], .. }` is not equal to
  `FnSig { inputs: vec![ty2, ty1], .. }` (input order matters).

### `FnSigInterner` smoke tests

- `FnSigInterner::new()` returns an empty interner.
- `intern(sig)` returns `FnSigId(0)`; second `intern` of the same
  `sig` returns `FnSigId(0)` (content-keyed dedup).
- `resolve(FnSigId(0))` returns the original `&sig`.
- `find(&sig)` returns `Some(FnSigId(0))` after interning.
- `len()` reflects the number of interned entries.
- `iter()` yields `(FnSigId, &FnSig)` pairs in insertion order.

### `FnSigInterner` per-distinction dedup

- Intern `FnSig { abi: FnAbi::Rust, ... }` then `FnSig { abi:
  FnAbi::C, ... }` (other fields equal); two distinct `FnSigId`s.
- Same for `c_variadic`, `safety`, `inputs`, `output`.

### `FnSigInterner` custom serde

- Round-trip a populated `FnSigInterner` (5 distinct sigs) through
  `serde_json`; assert `len()`, `resolve(id)`, `iter()` order
  preserved.
- Inject duplicate canonical content into the serialised form and
  assert deserialise rejects with a content-uniqueness error.
- Inject a dangling `TyId` reference in `inputs` (a `FnSig {
  inputs: vec![TyId(99)], ... }` against a model with
  `len(types) < 99`) and verify deserialise validation either
  succeeds (the raw interner doesn't bound-check `TyId`s -- that
  lives at `RegseqModel::validate`) or rejects per the
  documented contract.

### `canonical_fn_sig` byte encoding

Build a fixture with `RegseqModel`, intern needed `Ty`s, and
verify the byte output for representative signatures:

- `FnSig { inputs: vec![], output: bool_ty, abi: FnAbi::Rust,
  c_variadic: false, safety: Safety::Safe }`
  -> `[0, 0, 0, 0]` (input_count) ++ `<canonical(bool_ty)>` ++
  `[0]` (FnAbi::Rust) ++ `[0]` (c_variadic=false) ++ `[0]`
  (Safety::Safe).
  Total: `4 + 1 + 1 + 1 + 1 = 8` bytes for `Ty::Bool` output.
- `FnSig { inputs: vec![u32_ty, u32_ty], output: u32_ty, abi:
  FnAbi::C, c_variadic: true, safety: Safety::Unsafe }`
  -> `[2, 0, 0, 0]` ++ `<canonical(u32_ty)>` ++ `<canonical(u32_ty)>`
  ++ `<canonical(u32_ty)>` ++ `[1]` ++ `[1]` ++ `[1]`.

### `Ty::FnPtr` deep canonicaliser integration

If P1a-DS-04 left a `todo!()` stub for the tag-12 row, this task
removes the stub and inserts `canonical_fn_sig`. Test:

- `canonical_ty(Ty::FnPtr(sig_id))` produces `[0x0C] ++
  <canonical_fn_sig(model.sigs.resolve(sig_id), model)>`.
- The integration covers the previously-`#[ignore]`d test
  `// IGNORE-UNTIL: P1a-DS-06` from P1a-DS-04; this task removes
  the `#[ignore]` attribute on the relevant test once the
  `canonical_fn_sig` integration is wired.

### `CAbiShape` / `CAbiLowering` / `SubstReason` constructor tests

- `CAbiShape::Direct` constructs.
- `CAbiShape::Substituted { emit_as: u32_ty, reason:
  SubstReason::TransparentChain }` constructs.
- Same for the other 2 `SubstReason` variants (`TagScalar`,
  `TransparentChainAndTagScalar`).
- `CAbiLowering { inputs: vec![CAbiShape::Direct, CAbiShape::Direct],
  output: CAbiShape::Direct }` constructs.
- `CAbiLowering` round-trips through serde.

### `CAbiShape` / `CAbiLowering` equality / hash

- Two equal `CAbiLowering`s are equal and hash equal.
- Two `CAbiShape::Substituted { emit_as: t1, reason: r }` and
  `CAbiShape::Substituted { emit_as: t2, reason: r }` with t1 != t2
  are not equal.
- Same for `reason` field.

### Worked example: `Handle` repr(transparent) chain (spec.md:5454-5466)

Build a fixture model with:

- `Handle(u32)` (repr(transparent) over `u32`).
- `extern "C" fn(Handle) -> Handle` -- `FnSig { inputs:
  [TyId(Handle)], output: TyId(Handle), abi: C, c_variadic: false,
  safety: Safe }`.
- `sig_c_abi_lowerings[id] = Some(CAbiLowering {
    inputs: vec![CAbiShape::Substituted {
      emit_as: TyId(u32),
      reason: SubstReason::TransparentChain
    }],
    output: CAbiShape::Substituted {
      emit_as: TyId(u32),
      reason: SubstReason::TransparentChain
    }
  })`.

Verify:

- The FnSig stores `TyId(Handle)` (original Rust type), not
  `TyId(u32)`.
- The `CAbiLowering` stores `TyId(u32)` in `emit_as` for both
  positions.
- The two FnSigs `extern "C" fn(Handle) -> Handle` and
  `extern "C" fn(u32) -> u32` (with `CAbiLowering { inputs:
  vec![Direct], output: Direct }`) are **distinct** in the
  interner -- preserving rustc-distinct types per the type-identity
  rule.

### Closed-enum exhaustiveness scaffolds

`CAbiShape` and `SubstReason` each get a hidden
`_exhaustiveness_check` function with an exhaustive `match`.

### Sig-side stub removal in `model.rs`

If P1a-DS-01 introduced `FnSigInterner` as an empty stub
(`pub struct FnSigInterner;`), this task removes the stub and
re-exports the real `FnSigInterner` from `fn_sig_interner.rs`.
Verify the root struct's `pub sigs: FnSigInterner` field
type-checks against the real type.

## Review

After tests pass, request a Codex Task Review per `SESSION.md`
Review section. Use `scripts/python/codex_runner.py` directly via
the Bash tool with the four-block template:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-70 against its task spec at
.harness/tasks/regseq-model/70-fnsig-and-cabi-lowering.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/fn_sig.rs
- crates/regseq-model/src/fn_sig_interner.rs
- crates/regseq-model/src/cabi.rs
- crates/regseq-model/src/lib.rs (re-exports)
- crates/regseq-model/src/model.rs (sigs field replacement)
- crates/regseq-model/src/canonical.rs or wherever P1a-DS-04 placed
  the `Ty::FnPtr` tag-12 dispatch arm

Focus on:
- Spec deviations (every rule in the Implementation Notes section,
  including the spec-quoted public-API signatures: 5-field FnSig,
  6-method FnSigInterner, CAbiShape with 2 variants, CAbiLowering
  with 2 fields, SubstReason with 3 variants).
- `FnSig` Hash + Eq covers exactly the 5 schema fields.
- `c_variadic` / `Ty::Never` input invariants documented on the
  fields (enforcement at P1a-BR-03b / VAL-03).
- `Ty::FnPtr` integration: canonical_ty tag-12 arm calls
  canonical_fn_sig (no remaining todo!() stubs / IGNORE-UNTIL
  test markers).
- Custom serde for FnSigInterner uses storage-only on-wire shape.
- Closed-enum discipline on CAbiShape / SubstReason; no
  #[non_exhaustive], no wildcard arms.
- Type identity preservation: FnSig.inputs/output store original
  Rust TyIds; CAbiLowering's emit_as carries the C-side
  substitution per spec.md:5430-5475.
- Test coverage vs the task's Test Plan (per-field FnSig
  distinctions in the interner, custom serde, canonical_fn_sig
  byte encoding, Ty::FnPtr integration, CAbiLowering Handle
  worked example, exhaustiveness scaffolds).
Do NOT flag anything explicitly marked out of scope (Non-Goals:
P1a-BR-03b owns the recursive predicate; P1a-VAL-03 owns
sig_c_abi_lowerings length / arity / shape validation; P1a-DS-08b
owns vacant-vtable canonical signature; P1a-DS-10 owns
InstanceKind body).
</task>

<grounding_rules>
Cite file:line for every claim and every required fix. Do not
speculate about code you did not read. Mark hypotheses as
hypotheses.
</grounding_rules>

<structured_output_contract>
First line: VERDICT: APPROVED | REQUIRES FIXES
If REQUIRES FIXES: numbered list, each entry exactly
  <file>:<line-range> -- <what is wrong> -- <what the spec requires>
If APPROVED: one paragraph (<=120 words) summarizing what was
verified and why any deliberate deviations are acceptable.
Total response <= 400 words.
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, form a verdict,
stop.
</default_follow_through_policy>
EOF
```

Iterate until VERDICT: APPROVED.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- 5-field `FnSig` lands per spec.md:5258-5274 with derives that
  cover exactly the 5 schema fields under `Hash + Eq`.
- `FnSigInterner` lands with all 6 spec methods (`new`, `intern`,
  `resolve`, `find`, `len`, `iter`); custom serde with storage-only
  on-wire shape.
- `CAbiShape` (2 variants), `CAbiLowering` (2 fields),
  `SubstReason` (3 variants) land per spec.md:5395-5434.
- `canonical_fn_sig(sig: &FnSig, model: &RegseqModel) -> Vec<u8>`
  helper lands; the `Ty::FnPtr` tag-12 arm of P1a-DS-04's deep
  canonicaliser integrates this helper (no remaining `todo!()`
  stubs).
- `FnSig` construction invariants documented on the relevant
  fields (Never-input rejection, c_variadic implies FnAbi::C and
  inputs.len() >= 1, unit return admitted).
- `CAbiShape` and `SubstReason` exhaustiveness scaffolds present.
- All tests pass (`cargo test -p regseq-model`).
- Codex Task Review verdict: APPROVED.
