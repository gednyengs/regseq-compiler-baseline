# HIR/MIR Coverage Matrix

This note records Step 2 of the semantic procedural IR validation checklist.

The goal is to verify that every in-scope HIR construct and MIR body construct
has:

- an explicit semantic-model target
- or an explicit hard-error disposition

This note also carries forward the design gaps discovered in Step 1.

## Inputs

- [representative-function-trace-validation.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/representative-function-trace-validation.md)
- [pl011-full-spec-mir-study.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/pl011-full-spec-mir-study.md)
- final research plan at `~/.claude/plans/happy-crafting-muffin.md`

## HIR Coverage

| Source Construct | Extraction Source | Semantic-Model Target | Unsupported Behavior | Hard-Error Rule |
|------------------|-------------------|-----------------------|----------------------|-----------------|
| Module | HIR | `Module` | Recursive nesting deferred in first slice | No hard error in first slice if drivers are flat; nested modules are a deferred modeling extension |
| Struct | HIR | `StructDef` | None in current slice | Unsupported struct features become explicit model extensions, not backend inference |
| Struct field | HIR | `StructFieldDef` | None in current slice | Unknown field types become `SemanticType::Opaque` and must not reach backend emission |
| Enum | HIR | `EnumDef` | Deferred population in first implementation slice | If an enum is needed by executable semantics before support lands, semantic lowering must fail explicitly |
| Impl | HIR | `ImplDef` | None in current slice | Missing impl ownership/provenance is a lowering error |
| Free function | HIR + MIR | `FunctionDef` with `FunctionKind::FreeFunction` | None in current slice | Missing body/signature linkage is a lowering error |
| Inherent method | HIR + MIR | `FunctionDef` with `FunctionKind::InherentMethod` | None in current slice | Missing owner link is a lowering error |
| Trait method | HIR + MIR | `FunctionDef` with `FunctionKind::TraitMethod` | None in current slice | Missing trait provenance is a lowering error |
| ISR marker | HIR | `FunctionRole::Isr { binding }` | Only driver-owned inherent methods may be ISR roots | Any non-driver or non-annotated method assigned ISR role is a validation error |
| Associated const | HIR | `ConstantDef` + `ConstantRef` | None in current slice | Missing canonical identity or owner is a lowering error |
| Type alias | HIR | `TypeAliasDef` | Deferred in first slice | If a type alias is required semantically before support lands, fail explicitly |
| Register block annotation | HIR | `StructDef` + register attributes | None in current slice | Missing register metadata is a declaration extraction error |
| Resource fields (`Mmio`, `Clock`, `Reset`, `PinCtrl`) | HIR | `StructFieldDef` + semantic field type + cross-link identity | None in current slice | Missing resource identity/provenance is a declaration extraction error |

## MIR Coverage

| Source Construct | Extraction Source | Semantic-Model Target | Unsupported Behavior | Hard-Error Rule |
|------------------|-------------------|-----------------------|----------------------|-----------------|
| Parameter locals | MIR + HIR | `SemanticLocal { origin: Param }` | None | Missing authored names is a lowering error, not a backend fallback |
| Authored locals | MIR | `SemanticLocal { origin: Authored }` | None | Unknown local typing/naming is a lowering error |
| Derived temporaries | MIR | `SemanticLocal { origin: Derived }` | None | Raw MIR temporaries must never survive into backend input |
| Return place | MIR | `SemanticLocal { origin: ReturnPlace }` + `Stmt::Return` | None | Raw `_0` conventions must never leak |
| Assignment | MIR | `Stmt::Let` or `Stmt::Assign` | None | Unsupported assignment forms fail in semantic lowering |
| Compound update | MIR | `Stmt::CompoundAssign` | None | If compound semantics cannot be proven, fail lowering |
| Arithmetic ops | MIR | `Expr::BinOp` / `Expr::UnaryOp` | None | Unsupported arithmetic forms fail lowering |
| Cast | MIR | `Expr::Cast` | None | Unsupported cast forms fail lowering |
| Helper call | MIR | `Expr::HelperCall` or `Stmt::Eval(HelperCall)` | None | Any unclassified call is a lowering error |
| Register write wrapper call | MIR | `Stmt::RegWrite` / `Stmt::RegModify` | None | Wrapper syntax must never survive; failed classification is a lowering error |
| Register read wrapper call | MIR | `Expr::RegReadRaw` / `Expr::RegFieldRead` / `Expr::FieldExtract` | None | Wrapper syntax must never survive; failed classification is a lowering error |
| Resource side-effect call | MIR | `Stmt::ResourceOp` | None | Unknown resource operation is a lowering error |
| Resource property read | MIR | `Expr::ResourcePropertyRead` | Only closed property set currently modeled | Unknown property read is a lowering error or model-extension trigger |
| Branch (`SwitchInt`) | MIR | `Stmt::Branch` | None | Unstructured branch residue is a lowering error |
| Loop CFG | MIR | `Stmt::Loop` | None | Back-edge that cannot be structured is a lowering error |
| Ordered barrier pair | MIR | `Stmt::OrderedRegion` | None for supported `ordered!` pattern | Unpaired barrier sequences are a lowering error |
| Fallible `?` chain | MIR | `Stmt::FallibleOp` | Richer error semantics deferred | If `Try` shape cannot collapse cleanly, fail lowering |
| Associated const reference | MIR | `Expr::ConstantRef` | None | Unevaluated consts without identity become lowering errors |
| Duration constructor | MIR | `ResourceOperation::Toggle { duration_us }` or other semantic absorption | Only supported constructors may be absorbed | Unsupported duration usage is a lowering error |
| `Option::as_ref().map(...).unwrap_or(...)` | MIR | `Expr::ResourcePropertyRead` + `Expr::WithDefault` or structured branch/value form | Only supported value chains may be normalized | Unsupported chains are lowering errors |
| `Option` presence test on resource | MIR | `Expr::ResourcePresent` | None in current resource-specific case | Missing resource identity is a lowering error |
| `Option` presence test on general value | MIR | **Gap: no explicit current target** | Needed for `rx_char().is_some()` and `if let Some(byte) = self.pending_tx` | Must become explicit semantic construct; backend must not infer truthiness |
| `Option` value construction / assignment | MIR | **Gap: no explicit current target** | Needed for `pending_tx = None` and future `Some(x)` cases | Must become explicit semantic construct; backend must not infer carrier values |
| Discriminant reads | MIR | Absorbed into semantic conditions | None in supported slice | Raw discriminant logic must not survive |
| Projection chains | MIR | Resolved semantic refs before IR emission | None in supported slice | Raw projection syntax must not survive |
| Deref chains | MIR | Absorbed into place resolution | None in supported slice | Raw deref wrapper syntax must not survive |
| StorageLive / StorageDead | MIR | eliminated | None | Never reaches backend |
| Basic block indices | MIR | eliminated by structured `Block` | None | Never reaches backend |
| Unwind / cleanup BBs | MIR | absorbed into `FallibleOp.cleanup_on_error` | Richer unwind semantics deferred | Unsupported cleanup semantics are lowering errors |

## Matrix Findings

### Covered cleanly

The current design covers most of the supported PL011 slice cleanly:

- HIR structural items
- function ownership and semantic role
- local environment
- arithmetic/value temps
- register operations
- resource operations
- resource property reads
- interrupt tests
- fallible flow
- ordered regions
- associated constants

### Gaps confirmed

Step 1 surfaced two option-carrier gaps. Step 2 confirms they are not isolated
quirks; they are genuine holes in the current expression/value design.

#### Gap 1: general optional-value presence testing

The design currently has:

- `Expr::ResourcePresent`

but it does not have a general carrier-value presence test for:

- helper results returning `Option<T>`
- driver fields of type `Option<T>`
- local option-typed values

Affected real examples:

- `while self.rx_char().is_some()`
- `if let Some(byte) = self.pending_tx`

This is broader than a resource-specific issue. It is a missing general carrier
test in the expression language.

#### Gap 2: explicit option-like carrier values

The design currently has `SemanticType::Option { inner }`, but it does not have
corresponding expression/value forms for:

- `None`
- `Some(expr)`

Affected real example:

- `self.pending_tx = None`

This means the type model and expression model are currently asymmetric:
- the type system knows option-like carriers exist
- the expression system cannot construct them explicitly

### Secondary concern: Result success-value encoding still needs tightening

The matrix does not show a structural gap here, but it does show a design area
that still needs specification:

- how `Result<(), E>` success is represented in `Stmt::Return`
- how `FallibleOp.cleanup_on_error` and the success path interact with carrier
  values

This is already recognized in the research plan as an open question, so it is
not a new blocker, but it should remain visible.

## Recommended Design Adjustment Before Implementation

The plan should be revised to add a small, explicit carrier-expression family.

At minimum, the expression language likely needs:

- `Expr::IsSome(Box<Expr>)`
- `Expr::OptionNone`
- `Expr::OptionSome(Box<Expr>)`

Possibly, if a more uniform carrier model is preferred:

- generalized carrier tests / constructors for both `Option` and `Result`

The important requirement is:

- backend code must not infer optional-value truthiness or construct carrier
  values from conventions

## Step 2 Verdict

Step 2 is **completed**.

Outcome:

- the design is broadly strong
- but the option-carrier gap from Step 1 is confirmed as a real semantic-IR
  deficiency, not a one-off lowering sketch issue
- no additional major gap of similar magnitude was discovered in the current
  PL011-supported slice
