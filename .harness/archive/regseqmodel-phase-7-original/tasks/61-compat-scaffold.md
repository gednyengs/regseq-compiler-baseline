# regseq-model-61: Phase-7 Compat Scaffold -- `crates/regseq-model/src/compat.rs`

## Description

Phase-7 opens with the compat adapter scaffold. This task creates
`crates/regseq-model/src/compat.rs`, a single bridge module that every
Phase-7 consumer migration (tasks `regseq-analysis-06`,
`regseq-lowering-10`, `regseq-linux-backend-03`, `regseq-driver-35`)
imports from. The module has two responsibilities:

1. **Re-export** every enum / small value type that current consumers
   import from `regseq_model::{declaration, binding, joined, semantic,
   item}` for matching and keying (`CallbackFamily`,
   `DeviceCallbackRole`, `FunctionRole`, `DeviceClass`, `DeviceClassKind`,
   `ItemId`, `LifecycleRole`, etc.) so consumer migrations become
   import-path-only diffs.
2. **Wrap** every structural old-model type the consumers read
   (`DeclarationModel`, `BindingModel`, `JoinedArtifact`, `SemanticModel`,
   `BusBinding`, and the subset of `Item`-tree projections callers use)
   in facade types that carry two constructors:
   `new_legacy(&legacy_type)` (used during Phase-7 by emit_driver while
   the old pipeline is still live) and `new_from_regseq_model(&RegseqModel)`
   (implemented in task `regseq-driver-35` when the runtime cutover
   happens).

This task implements **only the `new_legacy` arm** and stubs the
`new_from_regseq_model` arm with `unimplemented!("populated by
regseq-driver-35")` so the module compiles and downstream tasks can
migrate imports immediately. Task `regseq-driver-35` fills in the
`new_from_regseq_model` implementations as part of the runtime
cutover.

No consumer code changes. No `emit_driver.rs` changes. This task
ends with `compat.rs` present, `cargo test -p regseq-model` green,
and workspace-green unchanged.

## Input Context

- `.harness/spec-batches/regseqmodel-phase-7.md` (the brief; lines
  80-92 establish the adapter scope and "read-only views over
  `RegseqModel`" contract, lines 84-87 mandate the file lives only
  at this path, lines 121-123 forbid deletion of old modules).
- `docs/discussions/models/regseq-model/execution-plan.md` Phase 7
  block (lines 358-372) and the "Consumer migration cascade" risk
  row.
- `crates/regseq-model/src/model/regseq_model.rs` -- destination
  model.
- `crates/regseq-model/src/declaration/mod.rs` and
  `crates/regseq-model/src/declaration/traversal.rs` -- exact
  accessor method names the compat surface must mirror
  (`all_resource_field_names`, `all_call_sites`,
  `all_callback_impls`, `wrapper_call_sites`, `poll_call_sites`,
  `clock_fields`, `reset_fields`, `pinctrl_fields`, `guard_scopes`,
  `log_call_sites`, etc.). Read this file end-to-end before
  writing the compat surface.
- `crates/regseq-model/src/binding/mod.rs` -- `BindingModel`,
  `BusBinding`, per-kind binding types.
- `crates/regseq-model/src/joined/mod.rs` + `joined/artifact.rs`
  -- `JoinedArtifact`, `JoinOutcome`, per-kind join entry types
  (`PropertyJoinEntry`, `MmioJoinEntry`, `InterruptJoinEntry`,
  `ClockJoinEntry`, `ResetJoinEntry`, `PinCtrlJoinEntry`), and
  the `{properties,mmio,interrupts,clocks,resets,pinctrl}_{joined,decl_only,binding_only}`
  accessor family.
- `crates/regseq-model/src/semantic/mod.rs` -- `SemanticModel`,
  `SemanticBody`, `Block`, `Stmt`, `Expr`, `LocalId`,
  `LocalOrigin`, `SemanticType`, `ResourceOperation`,
  `ResourceRef`, `RegisterRef`, `FieldWriteValue`, `BinOp`,
  `UnaryOp`, `FunctionRef`, `ConstantRef`, `InterruptClassRef`,
  `SubsystemActionRef`, `DriverFieldRef`, `RegisterFieldRef`.
- `crates/regseq-model/src/item/mod.rs` -- `Item`, `ItemId`,
  `ItemStore`, `CanonicalPath`, `Module`, `FunctionDef`,
  `ConstantDef`, `StructDef`, `EnumDef`, `ImplDef`,
  `InterruptClassDef`, `FunctionKind`, `FunctionRole`,
  `LifecycleRole`, `DeviceCallbackRole`, `DeviceClassKind`,
  `StructFieldDef`, `EnumVariantDef`, `PrimitiveType`, `TypeRef`,
  `FieldAttribute`, `ItemAttribute`.
- `crates/regseq-analysis/src/input.rs`,
  `crates/regseq-analysis/src/readiness.rs`,
  `crates/regseq-analysis/src/body_readiness.rs` -- **the concrete
  callers** whose imports task `regseq-analysis-06` will swap to
  `compat::*`. Every type and accessor they reach for must be
  present in `compat`.
- `crates/regseq-backends/src/lowering/planner.rs`,
  `crates/regseq-backends/src/linux/planner.rs`,
  `crates/regseq-backends/src/linux/plan.rs` -- concrete callers for
  tasks `regseq-lowering-10` and `regseq-linux-backend-03`.
- `crates/regseq-model/src/lib.rs` -- the `pub mod compat;`
  declaration will land here.

## Implementation Notes

### Module layout

Single file: `crates/regseq-model/src/compat.rs`. No submodules.
Target size: ~600-1200 lines. If it exceeds that, split into
`crates/regseq-model/src/compat/{mod,views,reexports}.rs` and keep
the public path `regseq_model::compat::*` stable.

Add `pub mod compat;` to `crates/regseq-model/src/lib.rs`.

### Section A: Legacy type re-exports (by name)

The module re-exports every symbol that current consumers import
from `regseq_model::{declaration, binding, joined, semantic, item}`
(including nested submodules like `declaration::mir_body` and free
helper functions). A consumer migration task (2, 3, 4) substitutes
`regseq_model::compat::X` for `regseq_model::<old_module>::X` with
zero body changes; because `compat::X` IS the legacy type (via
`pub use`), owned field types in `AnalysisInput` and elsewhere stay
structurally identical.

The re-export table below is the **audited baseline** derived from
the actual imports in `crates/regseq-analysis/src/{input,readiness,body_readiness}.rs`,
`crates/regseq-backends/src/lowering/planner.rs`, and
`crates/regseq-backends/src/linux/{planner,plan}.rs`. Before
committing, re-run the audit command and add any additional
symbols:

```
rg -n 'regseq_model::(declaration|binding|joined|semantic|item)::[A-Za-z0-9_:]+' \
    crates/regseq-analysis/src/ \
    crates/regseq-backends/src/lowering/planner.rs \
    crates/regseq-backends/src/linux/planner.rs \
    crates/regseq-backends/src/linux/plan.rs \
    crates/regseq-driver/src/emit_driver.rs \
    tests/e2e/tests/
```

Every symbol that appears in the output must be re-exported below,
or the downstream migration tasks will fail their grep gates.

```rust
// ----- Structural types (owned; field-type friendly) -----
pub use crate::declaration::{
    DeclarationModel,
    DriverDecl,
    BusType,
    DeviceClass,
    CallbackFamily,
    CallbackDeclSet,
    CallbackImplDecl,
    BodyDeclSet,
    FnBodyDecl,
    MirBodyDeclSet,
    MirFnBodyDecl,
    MirQueryFlavor,
    PropertyLintDecl,
    ResourceDeclSet,
    SynchronizationDeclSet,
    RegisterDeclSet,
    PropertyDeclSet,
    TimedDelayDeclSet,
    LoggingDeclSet,
    PmGateDeclSet,
    WrapperCallSiteDecl,
    PollCallSiteDecl,
    FamilyCallSite,
    ClockFieldDecl,
    ResetFieldDecl,
    PinctrlFieldDecl,
    GuardScopeDecl,
    LogCallSiteDecl,
    // Add every other declaration:: symbol referenced anywhere
    // in the audited files above. Enumerate exhaustively; do not
    // assume the list is complete.
};

// ----- MIR body nested module re-exports -----
pub use crate::declaration::mir_body::{
    MirBody,
    MirOperand,
    MirPlace,
    MirProjectionElem,
    MirRvalue,
    MirStatementKind,
    MirTerminatorKind,
    // IMPORTANT: body_readiness.rs imports `FnKind` from
    // declaration::mir_body (the MIR flavor -- NOT the
    // authored-body FnKind under declaration::*). Re-export the
    // MIR flavor at the unqualified name `FnKind` so consumer
    // imports (`use regseq_model::compat::FnKind;`) resolve to the
    // same type they do today from
    // `regseq_model::declaration::mir_body::FnKind`. Deliberately
    // omit the declaration-root FnKind from compat: if a future
    // consumer needs it, re-export it under a distinct name such
    // as `AuthoredFnKind` to avoid the collision.
    FnKind,
    // Add every other declaration::mir_body:: symbol referenced by
    // body_readiness.rs or the planners.
};

// ----- Binding types -----
pub use crate::binding::{
    BindingModel,
    BindingHeader,
    BusBinding,
    JoinKey,
    PropertyBinding,
    BindingProvenance,
    MmioBinding,
    InterruptBinding,
    ClockBinding,
    ResetBinding,
    PinCtrlBinding,
    // Add every other binding:: symbol referenced by consumers.
};

// ----- Joined artifact types -----
pub use crate::joined::{
    JoinedArtifact,
    JoinedHeader,
    JoinOutcome,
    PropertyJoinEntry,
    MmioJoinEntry,
    InterruptJoinEntry,
    ClockJoinEntry,
    ResetJoinEntry,
    PinCtrlJoinEntry,
    // Add every other joined:: symbol referenced by consumers.
};

// ----- Semantic types -----
pub use crate::semantic::{
    SemanticModel,
    SemanticBody,
    Block,
    Stmt,
    Expr,
    SemanticLocal,
    LocalId,
    LocalOrigin,
    SemanticType,
    ResourceOperation,
    ResourceRef,
    RegisterRef,
    FieldWriteValue,
    BinOp,
    UnaryOp,
    FunctionRef,
    ConstantRef,
    InterruptClassRef,
    SubsystemActionRef,
    DriverFieldRef,
    RegisterFieldRef,
    // Add every other semantic:: symbol referenced by consumers.
};

// ----- Item-tree types -----
pub use crate::item::{
    Item,
    ItemId,
    ItemStore,
    CanonicalPath,
    Module,
    FunctionDef,
    ConstantDef,
    StructDef,
    EnumDef,
    ImplDef,
    InterruptClassDef,
    FunctionKind,
    FunctionRole,
    LifecycleRole,
    DeviceCallbackRole,
    DeviceClassKind,
    StructFieldDef,
    EnumVariantDef,
    PrimitiveType,
    TypeRef,
    FieldAttribute,
    ItemAttribute,
    // Add every other item:: symbol referenced by consumers.
};

// ----- Free helper functions consumers import -----
// The concrete public symbol is
// `crate::declaration::bodies::collect_call_targets` (not
// re-exported at declaration::). Either re-export the true path
// directly:
pub use crate::declaration::bodies::collect_call_targets;
// ...or first add `pub use bodies::collect_call_targets;` to
// declaration/mod.rs and re-export via declaration::; pick
// whichever keeps the downstream consumer code a pure path
// rewrite with no body churn.
// Add every other free-function symbol referenced from
// regseq_model::{declaration,binding,joined,semantic,item}:: by any
// consumer file in the audit set.
```

**Surface completeness rule (enforced)**: the implementer runs the
audit command above and every output symbol is covered by a `pub
use` here. If a new symbol is found, add it before committing. The
downstream task `regseq-analysis-06` will re-run this grep on the
consumer crate and fail if any symbol is still imported through the
legacy path.

### Section B: View types (prepared for task `regseq-driver-35`)

This section defines NEW read-only view types -- `DeclarationView`,
`BindingView`, `JoinedView`, `SemanticView`, `ItemView` -- that
will back `AnalysisInput::from_regseq_model` and the E2E rewrites
after task `regseq-driver-35` fills in the `new_from_regseq_model`
arms. During tasks 2-4 the views are not exercised on any hot path;
they exist to compile and to carry stubbed constructors so the
Phase-7 handoff contract is explicit.

Shape:

```rust
pub struct DeclarationView<'a> {
    backing: DeclBacking<'a>,
}
enum DeclBacking<'a> {
    Legacy(&'a crate::declaration::DeclarationModel),
    RegseqModel(&'a crate::RegseqModel),
}
impl<'a> DeclarationView<'a> {
    pub fn new_legacy(m: &'a crate::declaration::DeclarationModel) -> Self {
        Self { backing: DeclBacking::Legacy(m) }
    }
    pub fn new_from_regseq_model(m: &'a crate::RegseqModel) -> Self {
        // Stub: panics by design until regseq-driver-35 implements it.
        let _ = m;
        unimplemented!("regseq-driver-35 fills this in during the runtime cutover")
    }
    // ----- Delegating accessors (Legacy arm implemented here) -----
    pub fn all_resource_field_names(&self) -> Vec<(&'static str, &str)> {
        match &self.backing {
            DeclBacking::Legacy(m) => m.all_resource_field_names(),
            DeclBacking::RegseqModel(_) => unimplemented!("regseq-driver-35"),
        }
    }
    // ... every accessor currently called from regseq-analysis,
    // lowering/planner, linux/planner, linux/plan.rs. The accessor
    // list is derived by grepping the consumer files for method
    // calls on `DeclarationModel`-typed values. Enumerate
    // exhaustively; a missed accessor blocks task `regseq-driver-35`.
}
```

Repeat for `BindingView`, `JoinedView`, `SemanticView`, `ItemView`.
Each view's accessor list is derived from the consumer files by
grep. View types have **no type aliases**: `compat::DeclarationModel`
is the legacy re-export from Section A, NOT `DeclarationView`. This
separation avoids the field-type / borrow-lifetime collision that
would block `AnalysisInput` from keeping its current storage shape.

### Section C: The `unimplemented!()` contract

Every `new_from_regseq_model` and every `RegseqBacking` match arm
ends with `unimplemented!("regseq-driver-35 fills this in during
the runtime cutover")`. This is the explicit Phase-7 handoff: task
5 owns the replacement.

Guard the contract with a `#[test]` per view that calls
`new_from_regseq_model` on a minimal `RegseqModel` and uses
`#[should_panic(expected = "regseq-driver-35")]` -- so the stub
presence is asserted, and so task `regseq-driver-35` has to delete
these tests as part of its implementation.

### Section D: What this task does NOT do

- Does not modify any file in `crates/regseq-analysis/`,
  `crates/regseq-backends/`, `crates/regseq-driver/`, or
  `tests/e2e/`.
- Does not modify `crates/regseq-model/src/declaration/`,
  `binding/`, `joined/`, `semantic/`, or `item/`.
- Does not modify the Phase-6 pipeline
  (`crates/regseq-model/src/{normalize,join,verify}/`).
- Does not delete any symbol or change any public API on old
  modules.

## Non-Goals

- No consumer migration (tasks `regseq-analysis-06`,
  `regseq-lowering-10`, `regseq-linux-backend-03`).
- No runtime pipeline cutover (task `regseq-driver-35`).
- No E2E rewrite (task `track-a-validation-11`).
- No Phase-8 work (Linux emission rewrite).
- No Phase-9 work (old-module deletion).
- **Do not modify `crates/regseq-driver/src/construction*`.** The
  adapter does not touch the construction pipeline. Per brief
  line 121 (analogous to Phase-6 brief's strictest non-goal).
- No backend-specific legality checks.

## Test Plan

Unit tests under `crates/regseq-model/tests/compat_scaffold.rs`
(new file):

- **`compat_reexports_compile`**: dummy fn that `use`s every
  re-exported enum / value type from `compat::*`. Compile-only;
  catches missing symbols.
- **`declaration_view_legacy_round_trip`**: construct a minimal
  `DeclarationModel` programmatically (fixtures already used by
  earlier tasks), wrap in `DeclarationView::new_legacy`, call
  every accessor the consumers will call, assert each returns
  the same value the underlying model returns. Accessor list is
  the enumerated set from Section B.
- **`binding_view_legacy_round_trip`**: same pattern for
  `BindingView` on a minimal `BindingModel`.
- **`joined_view_legacy_round_trip`**: same pattern for `JoinedView`
  on a minimal `JoinedArtifact`.
- **`semantic_view_legacy_round_trip`**: same pattern for
  `SemanticView` on a minimal `SemanticModel`.
- **`item_view_legacy_round_trip`**: same pattern for `ItemView`
  on a minimal `ItemStore`.
- **`new_from_regseq_model_stub_panics`**: `#[should_panic]` test
  per view confirming the stub is present. Task `regseq-driver-35`
  deletes these tests in its implementation.
- **`compat_surface_grep`**: a `cargo xtask`-style test is
  out of scope; instead, a comment block at the top of
  `compat.rs` records the surface-completeness rule and points
  to the grep command the implementer ran to verify it. Task
  `regseq-analysis-06` re-runs that grep at its own acceptance.

## Review

After tests pass, invoke Codex directly per `.harness/SESSION.md`
Section "Review" (lines 39-102):

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Review regseq-model-61 compat scaffold against its spec at
.harness/tasks/regseq-model/61-compat-scaffold.md. ALL CHECKS MUST
BE SCOPED TO THE TASK.

Files to audit:
- crates/regseq-model/src/compat.rs
- crates/regseq-model/src/lib.rs (diff for the `pub mod compat;`)
- crates/regseq-model/tests/compat_scaffold.rs

Focus on:
- Re-export completeness: every enum/type consumers under
  crates/regseq-analysis/, crates/regseq-backends/,
  crates/regseq-driver/ import from
  regseq_model::{declaration,binding,joined,semantic,item} is
  re-exported from compat.
- Facade view API completeness: every method consumers call on the
  old structural types has a corresponding delegating accessor on
  the compat view.
- The `new_from_regseq_model` stubs are present on every view and
  uniformly panic with the Phase-7 handoff message.
- No modifications to crates/regseq-driver/src/construction*.
- No modifications outside the scope listed above.

Do NOT flag:
- Missing implementations of `new_from_regseq_model` (that is
  regseq-driver-35's work).
- Changes to consumer files (that is tasks 2-4).
</task>
... structured_output_contract per SESSION.md ...
EOF
```

Iterate until Codex returns APPROVED.

## Quality Gate

```
cargo test -p regseq-model --test compat_scaffold
cargo test -p regseq-model
cargo test --workspace        # still green; no behavior changes
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `crates/regseq-model/src/compat.rs` exists with three sections:
  (A) legacy type re-exports by name; (B) view types with
  `new_legacy` implemented and `new_from_regseq_model` stubbed;
  (C) the `unimplemented!()` handoff contract. There are no type
  aliases -- `compat::DeclarationModel` IS
  `crate::declaration::DeclarationModel` via `pub use`, not an
  alias to a view type.
- `crates/regseq-model/src/lib.rs` declares `pub mod compat;`.
- Every enum / value type referenced by any consumer under
  `crates/regseq-analysis/`, `crates/regseq-backends/`,
  `crates/regseq-driver/`, or `tests/e2e/` via
  `regseq_model::{declaration,binding,joined,semantic,item}::*`
  is re-exported from `compat`.
- Every method the consumers call on `DeclarationModel`,
  `BindingModel`, `JoinedArtifact`, `SemanticModel`, and the
  `Item`-tree projections has a delegating accessor on the
  corresponding compat view.
- `new_from_regseq_model` panics with the documented Phase-7
  handoff message on every view.
- Compat-scaffold tests pass.
- `cargo test --workspace` green.
- `git diff --name-only main..HEAD` touches only
  `crates/regseq-model/src/compat.rs`,
  `crates/regseq-model/src/lib.rs`,
  `crates/regseq-model/tests/compat_scaffold.rs`,
  and (possibly) `Cargo.lock`. No other files.
- Codex review approved.
