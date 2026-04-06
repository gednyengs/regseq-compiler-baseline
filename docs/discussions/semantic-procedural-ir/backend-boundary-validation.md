# Backend Boundary Validation

This note records Step 4 of the semantic procedural IR validation checklist.

The goal is to validate that the proposed backend boundary is actually clean:

- backends receive enough semantic information to emit code
- backends do not need to reconstruct Rust/MIR meaning
- unsupported semantic residue cannot silently leak across the boundary

## Inputs

- final research plan at `~/.claude/plans/happy-crafting-muffin.md`
- [representative-function-trace-validation.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/representative-function-trace-validation.md)
- [hir-mir-coverage-matrix.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/hir-mir-coverage-matrix.md)
- [ownership-and-identity-stress-test.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/ownership-and-identity-stress-test.md)

## What the backend is supposed to receive

Per the plan, backends consume:

- `DriverLoweringPlan`
  - `identity`
  - `resources`
  - `register_obligations`
  - `behavior: SemanticModel`

And `SemanticModel` contains:

- `functions: Vec<FunctionDef>`
- `interrupt_classes: Vec<InterruptClassDef>`

For each function body, the backend sees:

- explicit locals with:
  - names
  - types
  - origins
- explicit structured statements
- explicit structured expressions
- explicit register operations
- explicit resource operations
- explicit constant references with stable emitted names

## Boundary Sufficiency Check

### 1. Locals and names

The plan gives backends:

- `SemanticLocal`
  - `id`
  - `name`
  - `ty`
  - `origin`

This is sufficient for:

- declaration emission
- use/def tracking in emitted code
- avoiding any backend dependence on MIR-local names

Verdict:
- sufficient

### 2. Control flow

The plan gives backends:

- `Stmt::Branch`
- `Stmt::Loop`
- `Stmt::OrderedRegion`
- `Stmt::FallibleOp`
- `Stmt::Return`

This is sufficient for:

- structured C emission
- preserving control-flow intent without basic-block reconstruction

Verdict:
- sufficient

### 3. Register semantics

The plan gives backends:

- `Stmt::RegWrite`
- `Stmt::RegModify`
- `Expr::RegReadRaw`
- `Expr::RegFieldRead`
- `Expr::FieldExtract`
- register obligations in `DriverLoweringPlan`

This is sufficient for:

- mapping semantic register actions to OS/backend-specific helper calls
- avoiding direct backend reasoning over wrapper methods like
  `ReadWrite::<...>::write`

Verdict:
- sufficient

### 4. Resource semantics

The plan gives backends:

- `Stmt::ResourceOp`
- `Expr::ResourcePropertyRead`
- `Expr::ResourcePresent`
- `Expr::WithDefault`
- resource obligations in `DriverLoweringPlan`

This is sufficient for:

- mapping resource lifecycle and property access to Linux/Zephyr APIs
- avoiding backend reasoning over Rust wrapper APIs like `Clock::enable` or
  `PinCtrl::apply`

Verdict:
- sufficient

### 5. Helper and callback semantics

The plan gives backends:

- `FunctionDef.kind`
- `FunctionDef.role`
- `Expr::HelperCall`

This is sufficient for:

- distinguishing helper bodies from callback roots
- backend dispatch over callback roles
- helper-call emission without Rust-path inspection

Verdict:
- sufficient

### 6. Constants and stable names

The plan gives backends:

- `ConstantRef.canonical_path`
- `ConstantRef.stable_name`
- `ConstantRef.known_value`
- `ConstantDef.stable_name`

This is sufficient for:

- deterministic emitted `#define` names
- avoiding backend path parsing or short-name disambiguation

Verdict:
- sufficient

### 7. Carrier semantics

With the plan update, the backend now receives explicit option-carrier semantics:

- `IsSome`
- `OptionNone`
- `OptionSome`
- `OptionUnwrap`

This is sufficient for:

- emitting option-related control flow and assignment without implicit truthiness
- avoiding backend reconstruction of `Option` behavior from MIR discriminants

Verdict:
- sufficient for the current PL011-supported slice

## What the backend is explicitly protected from

Per the plan, backends never see:

- raw MIR locals like `_3`
- projections
- basic blocks
- terminators
- wrapper method paths such as:
  - `regseq::ReadWrite::<...>::write`
  - `regseq::Clock::enable`
- `core::option::Option` method spellings
- `core::ops::Try` machinery
- raw rustc type spellings
- raw return-place conventions
- cleanup block indices
- storage markers

This is the correct contract.

## Boundary Strength Test

### Question: can a backend emit code from this model without re-walking raw model data?

For the supported PL011 slice, the answer is:

- **yes**, with one important constraint

The constraint is:

- unsupported semantic types and unresolved semantic constructs must be stopped
  before they cross the boundary

The plan already says:

- unsupported MIR constructs are hard errors
- `Opaque` is diagnostic-only and must not reach backend emission

That rule is essential. Without it, the backend boundary would weaken
immediately because backends would be forced to recover meaning from unresolved
semantic residue.

### Question: does the backend still have to make target-language decisions?

Yes, but that is appropriate.

Examples:

- `SemanticType -> C type string`
- `ResourceOp -> Linux API call vs Zephyr API call`
- file layout
- registration scaffolding

Those are target realization decisions, not semantic recovery.

That means the boundary is clean:

- semantics are already decided before backend consumption
- realization is left to the backend

## Findings

### Strengths

The backend boundary is strong because:

- executable semantics are explicit
- ownership/role metadata are explicit
- register/resource behavior are semantic, not wrapper-shaped
- constant identity is explicit
- option-carrier semantics are now explicit
- the boundary explicitly forbids MIR/Rust residue

### Constraint to preserve

The boundary remains clean only if the compiler enforces:

- no `Opaque` types in backend-consumed semantic bodies for supported drivers
- no unresolved semantic constructs in `Stmt` / `Expr`
- no backend-side fallback that attempts to recover meaning from names, paths,
  or debug strings

This is not a new gap, but it is an important validation result:
- the design is good enough
- but only if semantic lowering is strict

## Step 4 Verdict

Step 4 is **completed**.

Outcome:

- the backend boundary is clean and sufficiently strong for the supported slice
- no new architectural blocker was discovered
- the most important condition is that semantic lowering must enforce the
  no-residue rule strictly before data reaches the backend
