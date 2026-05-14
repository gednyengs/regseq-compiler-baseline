# regseq-model-75: Instance, InstanceKind, InstanceId, FunctionInstanceFlavor (P1a-DS-10)

## Description

Replace the legacy `Instance` / `InstanceKind` shell at
`crates/regseq-model/src/instance.rs` with the locked phase-1
shape per `spec.md` §9.1-§9.4. The Option-B reframe (stage-12
D-12-1.4) lives Instance-side: post-mono substituted `fn_sig`
*and* `body` live on `InstanceKind::Function`, NOT on the parent
Item. The `InstanceKind::Function` variant therefore carries
four fields -- `fn_sig: FnSigId`, `body: Option<Body>`,
`flavor: FunctionInstanceFlavor`, `virtual_idx: Option<u32>` --
not the legacy two-field `{ sig, body: Option<BodyId> }` shape.

This task also pins the `FunctionInstanceFlavor` enum (the
structured form of the rustc-shim discriminator used inside
`RegseqInstanceKey`), the F4-2 `RegseqInstanceKey` interning-key
shape, the `Instance` struct's `emission_name` rename (replacing
the legacy `symbol_name`), and the `InstanceKind::as_function()`
hybrid accessor returning `&FunctionInstanceData`.

## Input Context

- `docs/discussions/regseq-model/spec.md` §9.1 (`GenericArg`,
  `GenericArgs`; `spec.md:2402-2415`)
- `docs/discussions/regseq-model/spec.md` §9.2 (`Instance` +
  `InstanceId`; `spec.md:2417-2476`)
- `docs/discussions/regseq-model/spec.md` §9.3 (Instance interning
  key + rank tables; `spec.md:2477-2575`)
- `docs/discussions/regseq-model/spec.md` §9.4 (`InstanceKind` +
  `FunctionInstanceFlavor` + `FunctionInstanceData`;
  `spec.md:2576-2771`)
- `docs/discussions/regseq-model/spec.md` §3.1 invariant 14
  (InstanceKind layout matches Ty nominal variant -- enforcement
  is validator-side P1a-VAL-03)
- `docs/discussions/regseq-model/decisions.md` D-12-1.4 (Option B
  rationale)
- `crates/regseq-model/src/instance.rs` (existing legacy shell;
  **must be amended** -- the `InstanceKind::Function { sig,
  body: Option<BodyId> }` legacy shape at lines 47-50 is
  superseded; the `Instance.symbol_name` field at line 88
  becomes `emission_name` per `spec.md:2423`)
- `crates/regseq-model/src/model.rs` (`validate_instance` at
  lines 326-345 reads the legacy shape; this task's amendments
  will require the validator to compile against the new shape)
- `crates/regseq-model/src/ids.rs` (`ItemId`, `FnSigId`,
  `ConstId`, `TyId`, `StringId`)
- `crates/regseq-model/src/source.rs` (`Span`)
- `.harness/spec-batches/regseqmodel-phase-1a.md` (parent brief;
  P1a-DS-10 row -- explicitly notes the existing shell must be
  amended)

## Implementation Notes

Amend `crates/regseq-model/src/instance.rs` -- this is **not** a
net-new file. The legacy `InstanceKind::Function { sig, body:
Option<BodyId> }` shape is superseded by the four-field Option-B
shape; the `Instance.symbol_name` field is renamed to
`emission_name`; the `RustcInstanceKindTag` enum (currently used
as a tag relaying rustc's `ty::InstanceKind` variant to the F4-2
sort) is renamed to `FunctionInstanceFlavor` and stored on
`InstanceKind::Function` as a public field per
`spec.md:2655-2671`.

The `Body` type lives in regseq-model-82 (P1a-DS-14b); this task
**uses `Body` as an opaque type** and depends on regseq-model-82
landing first. Because the brief's dependency declaration lists
this task as depending on regseq-model-71 only (the §3 root), the
worker MUST follow the actual dependency: regseq-model-82 must
ship before this one is started. (See the brief's row for
P1a-DS-10 -- "Depends on: regseq-model-68, -69, -70, -71" --
plus the Option-B requirement to embed `body: Option<Body>` in
`InstanceKind::Function`.)

### Public API -- GenericArg and GenericArgs

Per `spec.md:2402-2415`:

```rust
pub enum GenericArg {
    Type(TyId),
    Const(ConstId),
    // Lifetimes erased post-monomorphisation; not represented.
}

pub type GenericArgs = Vec<GenericArg>;
```

(The existing module already ships `GenericArg` and
`GenericArgs` at lines 24-34; preserve verbatim.)

### Public API -- Instance and InstanceId

Per `spec.md:2417-2476`:

```rust
pub struct Instance {
    pub item:          ItemId,        // origin: source-level item, always present
    pub generic_args:  GenericArgs,   // resolved args; empty for non-generic items
    pub emission_name: StringId,      // C-side identifier; uniqueness across `instances`
    pub span:          Span,          // SYNTHESIZED for bridge-synthesised items
    pub kind:          InstanceKind,
}

newtype_index! { pub struct InstanceId {} }
```

**Field rename: `symbol_name` -> `emission_name`** per
`spec.md:2423`. The C-side identifier semantic is preserved;
only the field name changes to reflect the spec's locked vocab.

### Public API -- InstanceKind (six-variant closed Option-B set)

Per `spec.md:2578-2671`:

```rust
pub enum InstanceKind {
    /// Function instantiation. Per stage-12 D-12-1.4 (Option B,
    /// restoring symmetry with non-Function variants):
    /// post-mono substituted `fn_sig` and `body` live HERE on
    /// `InstanceKind::Function`, NOT on the parent Item.
    Function {
        fn_sig:      FnSigId,
        body:        Option<Body>,
        flavor:      FunctionInstanceFlavor,
        virtual_idx: Option<u32>,
    },

    /// Static. `init` is `None` for foreign statics
    /// (`extern "C" { static FOO: u32; }`) and `Some(c)` for
    /// source-defined statics with an initial value. Mutability
    /// lives on `ItemKind::Static { mutability }` per stage-12
    /// D-12-1.2 (c).
    Static { ty: TyId, init: Option<ConstId> },

    /// Const item.
    Const { ty: TyId, value: ConstId },

    /// Nominal struct. `layout` -> `Ty::Struct`.
    Struct { layout: TyId },

    /// Nominal enum. `layout` -> `Ty::Enum`.
    Enum { layout: TyId },

    /// Nominal C-style union. `layout` -> `Ty::Union`.
    Union { layout: TyId },
}
```

**Differences from the legacy shell at lines 43-71**:
- `Function` now carries 4 fields (`fn_sig`, `body`, `flavor`,
  `virtual_idx`), not 2 (`sig`, `body: Option<BodyId>`).
- `body: Option<Body>` (structural, per Option B), not
  `body: Option<BodyId>` (referential, per the legacy
  separate-bodies-table framing).
- `Static.init: Option<ConstId>` (foreign-static carve-out per
  `spec.md:2625-2635`), not `init: ConstId` (legacy
  always-present).

### Public API -- FunctionInstanceFlavor (15 variants)

Per `spec.md:2655-2671` (mirrors rustc's `ty::InstanceKind`
discriminants for the Function Regseq kind; append-only):

```rust
pub enum FunctionInstanceFlavor {
    Item,
    Intrinsic,
    VTableShim,
    ReifyShim,
    FnPtrShim,
    Virtual,
    ClosureOnceShim,
    ConstructCoroutineInClosureShim,
    ThreadLocalShim,
    FutureDropPollShim,
    DropGlue,
    CloneShim,
    FnPtrAddrShim,
    AsyncDropGlueCtorShim,
    AsyncDropGlue,
}
```

The variant order is normative -- it is the rank-table mapping
`rustc_instance_kind_sort_rank(flavor) -> u8` per
`spec.md:2530-2555`. Preserve the existing
`rustc_instance_kind_sort_rank` const fn at lines 149-167 of
`instance.rs`, but route it through `FunctionInstanceFlavor`
rather than the legacy `RustcInstanceKindTag` (rename the enum;
the variant order is unchanged).

### Public API -- FunctionInstanceData and as_function() hybrid

Per `spec.md:2696-2718`:

```rust
pub struct FunctionInstanceData {
    pub fn_sig:      FnSigId,
    pub body:        Option<Body>,
    pub flavor:      FunctionInstanceFlavor,
    pub virtual_idx: Option<u32>,
}

impl InstanceKind {
    pub fn as_function(&self) -> Option<&FunctionInstanceData> { /* ... */ }
    pub fn as_function_mut(&mut self) -> Option<&mut FunctionInstanceData> { /* ... */ }
}
```

D-12-1.6 hybrid pattern: convenience methods `as_function() ->
Option<&FunctionInstanceData>` extract the struct-variant into a
named struct for ergonomic destructuring. Match arms remain
available for exhaustive handling.

**Implementation note for `as_function`.** The straightforward
implementation re-projects the named-fields-variant payload via
a per-call construction of `FunctionInstanceData`, but
`Option<&FunctionInstanceData>` requires a borrow of the variant
payload itself. The cleanest shape is to store a
`FunctionInstanceData` *as* the inline struct payload of
`InstanceKind::Function` -- i.e., flip the variant from
`Function { fn_sig, body, flavor, virtual_idx }` to
`Function(FunctionInstanceData)`. Either shape is admissible per
the spec; the spec's variant body in `spec.md:2619-2624` shows
named fields, so prefer the named-fields shape and implement
`as_function()` by returning a `FunctionInstanceData` reference
constructed via `unsafe { transmute }` over a `repr(C)` newtype
**only if `cargo clippy -- -D warnings` permits**; otherwise,
re-shape the variant to `Function(FunctionInstanceData)` and
expose accessors `fn_sig()`, `body()`, `flavor()`,
`virtual_idx()` for parity with the named-fields surface (cite
this choice in the worker's Codex review submission).

### Public API -- F4-2 RegseqInstanceKey

Per `spec.md:2482-2510`, the bridge interns instances during
construction with this key shape. The key itself is internal to
the bridge (not stored on `Instance`), but the shape is
data-shape: expose it as a `pub struct` so `regseq-driver`
(P1a-BR-01) can reuse it without re-defining.

```rust
/// Bridge-side instance interning key. Internal to instance
/// construction; see spec.md:2482-2510 for the rationale and
/// the rank-table mapping.
pub struct RegseqInstanceKey {
    pub regseq_kind_rank:   u8,    // Regseq InstanceKind variant rank
    pub item:               ItemId,
    pub generic_args_key:   ArgsKey,
    pub fn_shim_disambig:   u8,    // 0 for non-Function variants
}
```

(`ArgsKey` from §12.3; if the type is not yet available in the
foundation, use `u128` as the underlying byte form per
`spec.md:4845`'s precedent and document the deferral.)

### Conventions

- **`Instance.emission_name` is the locked spec name** per
  `spec.md:2455-2474`; the field replaces the legacy
  `symbol_name`. Update every reader (notably
  `model.rs:326-345`'s validator).
- **`Static.init` becomes `Option<ConstId>`** to accommodate
  foreign statics per `spec.md:2625-2635`. Update the validator
  at `model.rs:347-350` to handle the `Option`.
- **`Function.body` becomes `Option<Body>`** (structural Option-B
  per D-12-1.4) -- not `Option<BodyId>`. Update the validator
  at `model.rs:339-345` to walk `body: Option<&Body>` rather
  than checking `BodyId` membership in a separate table. The
  detailed body-content walk is P1a-VAL-01 / P1a-VAL-02; this
  task only adjusts the surface signature so existing validator
  code compiles.
- **`Function.flavor` is mandatory** per `spec.md:2608-2618`;
  bridge-synthesised Function Items use `flavor =
  FunctionInstanceFlavor::Item` by convention per
  `spec.md:2746-2765`.
- **`Function.virtual_idx` is `Some(idx)` iff `flavor ==
  Virtual`** per `spec.md:2720-2734` (construction invariant).
  Validator-side enforcement is P1a-VAL-03.
- **`regseq_instance_kind_rank` const fn** (existing at
  `instance.rs:100-109`) preserved verbatim; the rank table is
  append-only and matches the spec's normative rank table at
  `spec.md:2517-2528`.
- **Sigil conventions / `Debug` impl** stay per the project's
  stage-1 ID convention (auto-generated `InstanceId(N)`).
- **Serde**: `Instance`, `InstanceKind`, `FunctionInstanceFlavor`,
  `GenericArg`, `RegseqInstanceKey` carry typed IDs and `u8`s;
  no `Symbol`. Serde derives are admissible. `body: Option<Body>`
  inherits `Body`'s serde shape (which is deferred per the
  brief's cross-cutting note for symbol-bearing content); when
  `Body` is `#[serde(skip)]`, the surrounding `Option<Body>`
  serialises as `None`. Mirror the existing serde discipline
  on `instance.rs` lines 24, 42, 84.
- **Doc comments mandatory** on every public type, variant,
  field, and function.

## Non-Goals

- The body-content walk + the Option-B `body: Option<Body>` walk
  itself land in P1a-VAL-01 (Place validator) and P1a-VAL-02
  (ID referential integrity). This task only adjusts the
  surface signature to type-check.
- Bridge-side instance interning + the verification requirement
  on `RegseqInstanceKey` injectivity (`spec.md:2561-2574`) is
  P1a-BR-01.
- The Phase-2 / Phase-5 Function-Instance body materialisation
  pipeline is bridge-side (P1a-BR-07).
- The `instance_intrinsic` / pull-through helpers per
  `spec.md:1837-1842` (D-14-1.6) are out of scope; if needed by
  P1a-PP-05a, defer to a follow-up.
- The `Body` type itself is regseq-model-82 (P1a-DS-14b);
  this task uses `Body` as an opaque type. Order the
  implementation accordingly.
- Validation that `InstanceKind` variant matches the `Ty`
  nominal kind for Struct/Enum/Union (invariant 14) is
  P1a-VAL-03.

## Test Plan

**Test organization**: `unit-in-src` -- tests live in
`crates/regseq-model/src/instance.rs` `#[cfg(test)] mod tests`.

Coverage requirements (every public type / function / accessor
exercised at least once per `SESSION.md` Testing Requirements):

1. **`GenericArg::Type` / `GenericArg::Const` construction.**
   Build one of each; assert distinctness; clone equality.
2. **`Instance` construction + field access.** Build an
   `Instance { item, generic_args, emission_name, span, kind }`
   for a `Function` kind; assert each field reads back the
   construction value.
3. **`Instance.emission_name` rename verified.** Construct the
   struct using the field name `emission_name` (not
   `symbol_name`). Compile failure if the legacy name persists.
4. **`InstanceKind::Function` construction.** Build with a
   non-`None` `body: Option<Body>`, `flavor =
   FunctionInstanceFlavor::Item`, `virtual_idx = None`. Assert
   field-by-field reads.
5. **`InstanceKind::Function` -- foreign declaration shape.**
   Build with `body: None`, `flavor = Item`, `virtual_idx =
   None`. Assert the `Option<Body>` is `None`.
6. **`InstanceKind::Function` -- Virtual flavor.** Build with
   `flavor = FunctionInstanceFlavor::Virtual`, `virtual_idx =
   Some(0)`. Assert the pair invariant holds (this is a
   structural-construction test only; validator enforcement
   lives in P1a-VAL-03).
7. **`InstanceKind::Static` -- source-defined.** Build with
   `init: Some(const_id)`; assert.
8. **`InstanceKind::Static` -- foreign declaration.** Build with
   `init: None`; assert.
9. **`InstanceKind::Const` construction.** Build; assert.
10. **`InstanceKind::Struct` / `Enum` / `Union`.** Build each
    with a layout `TyId`; assert pairwise distinctness.
11. **`InstanceKind::as_function` -- positive.** Build a
    `Function`-kind variant; assert
    `kind.as_function().is_some()` and the destructured fields
    match.
12. **`InstanceKind::as_function` -- negative.** For each non-
    Function variant (Static, Const, Struct, Enum, Union),
    assert `kind.as_function().is_none()`.
13. **`InstanceKind::as_function_mut` -- positive + mutation.**
    Mutate `virtual_idx` from `None` to `Some(3)` via
    `as_function_mut`; assert subsequent
    `kind.as_function().unwrap().virtual_idx == Some(3)`.
14. **`FunctionInstanceFlavor` -- all 15 variants distinct.**
    Build each variant; assert pairwise inequality.
15. **`rustc_instance_kind_sort_rank` -- 0..=14 mapping.**
    Assert the existing rank function returns exactly the
    expected `u8` per `spec.md:2517-2528`'s table for each of
    the 15 variants. Drives total coverage of the 15-arm
    match.
16. **`regseq_instance_kind_rank` -- 0..=5 mapping.** Assert
    rank values match `spec.md:2517-2528`:
    `Function=0, Static=1, Const=2, Struct=3, Enum=4, Union=5`.
17. **`RegseqInstanceKey` construction + equality.** Build two
    keys with identical fields; assert `==`. Differ on each
    field one at a time; assert inequality.
18. **`InstanceId` typed-index size.** Assert
    `size_of::<InstanceId>() == 4` and
    `size_of::<Option<InstanceId>>() == 4` (per stage-1 niche
    convention).
19. **Serde round-trip -- `GenericArg` (each variant).**
20. **Serde round-trip -- `InstanceKind` (each variant).** Use
    a stand-in `Body` if `Body` is `#[serde(skip)]`; document
    the deferral in the test comment.
21. **Serde round-trip -- `FunctionInstanceFlavor` (each
    variant).**
22. **Serde round-trip -- `Instance`** -- top-level struct.
23. **Serde round-trip -- `RegseqInstanceKey`.**
24. **Validator compatibility.** Confirm
    `crates/regseq-model/src/model.rs::validate_instance`
    still compiles after the surface changes (rename
    `symbol_name -> emission_name`, `init: ConstId -> init:
    Option<ConstId>`, `sig -> fn_sig`, `body: Option<BodyId> ->
    body: Option<Body>`). The detailed walk-content tests live
    in P1a-VAL-02; this task only verifies surface
    type-checking.

`cargo test -p regseq-model` passes.

## Review

After tests pass, request a Codex review using the four-block
template:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-75 against its task spec at
.harness/tasks/regseq-model/75-instance-and-instance-kind.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/instance.rs
- crates/regseq-model/src/model.rs (validator-compatibility check only)
- crates/regseq-model/src/lib.rs

Focus on:
- Spec deviations (every rule in the Implementation Notes section).
- Correctness issues and generality loss.
- Design misalignment with the spec's public API / architecture.
- Test coverage vs. the spec's Test Plan.
- The legacy-vs-spec rename / shape changes (emission_name,
  Option<ConstId>, Option<Body>, FunctionInstanceFlavor naming).
Do NOT flag anything explicitly marked out of scope (Non-Goals,
later task IDs).
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

The authoritative gate is `task.py done` per `SESSION.md`.

## Acceptance Criteria

- `crates/regseq-model/src/instance.rs` amended (not replaced)
  to land:
  - `Instance` with `emission_name: StringId` field rename per
    `spec.md:2423`.
  - `InstanceKind::Function { fn_sig, body, flavor,
    virtual_idx }` four-field Option-B shape per
    `spec.md:2619-2624`.
  - `InstanceKind::Static { ty, init: Option<ConstId> }` per
    `spec.md:2635`.
  - The remaining four `InstanceKind` variants
    (`Const` / `Struct` / `Enum` / `Union`) per
    `spec.md:2637-2647`.
  - `FunctionInstanceFlavor` 15-variant enum per
    `spec.md:2655-2671` (renamed from
    `RustcInstanceKindTag`).
  - `FunctionInstanceData` struct + `as_function` /
    `as_function_mut` accessors per `spec.md:2702-2714`.
  - `RegseqInstanceKey` exported `pub struct` per
    `spec.md:2482-2490`.
- `crates/regseq-model/src/model.rs::validate_instance`
  recompiles against the new surface (no validator semantics
  changes -- those land in P1a-VAL-02 / P1a-VAL-03).
- All public types carry doc comments.
- Test Plan items 1-24 implemented; all pass under
  `cargo test -p regseq-model`.
- `cargo fmt --all -- --check` and
  `cargo clippy -p regseq-model -- -D warnings` pass.
- Codex Task Review returns VERDICT: APPROVED.
