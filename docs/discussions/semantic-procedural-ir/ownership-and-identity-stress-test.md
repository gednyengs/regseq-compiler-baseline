# Ownership and Identity Stress Test

This note records Step 3 of the semantic procedural IR validation checklist.

The goal is to test whether the proposed ownership and identity model remains
unambiguous under overlapping names, multiple impls, associated items, and
non-driver helper ownership.

## Inputs

- final research plan at `~/.claude/plans/happy-crafting-muffin.md`
- [representative-function-trace-validation.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/representative-function-trace-validation.md)
- [hir-mir-coverage-matrix.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/hir-mir-coverage-matrix.md)

## Stress Cases

### 1. Two different types with the same short method name

Test shape:

```text
impl DriverA {
    fn init(&mut self) { ... }
}

impl DriverB {
    fn init(&mut self) { ... }
}
```

Evaluation:
- short names collide
- `FunctionDef.name` alone is insufficient
- the design resolves this through:
  - `FunctionKind::InherentMethod { owner: ItemId }`
  - `ImplDef.self_type`
  - `CanonicalPath`

Expected canonical paths:

```text
pkg::driver_a::DriverA::init
pkg::driver_b::DriverB::init
```

Verdict:
- resolved cleanly
- no short-name heuristic needed

### 2. Multiple impls containing overlapping method names

Test shape:

```text
impl Driver {
    fn configure(&mut self) { ... }
}

impl Serial for Driver {
    fn configure(&mut self, cfg: &SerialConfig) -> Result<..., ...> { ... }
}
```

Evaluation:
- overlap exists at the short-name level
- structural distinction comes from:
  - `FunctionKind::InherentMethod`
  - `FunctionKind::TraitMethod { trait_ref, owner }`
- grouping distinction comes from:
  - separate `ImplDef` items
- identity distinction comes from:
  - distinct `CanonicalPath`

Verdict:
- resolved cleanly
- the separation of `FunctionKind` and `FunctionRole` is doing useful work here

### 3. Associated constants with the same short name on different owners

Test shape:

```text
impl DriverA {
    const DEFAULT_SPEED: u32 = ...;
}

impl DriverB {
    const DEFAULT_SPEED: u32 = ...;
}
```

Evaluation:
- short names collide
- the model resolves this through:
  - `ConstantDef.owner: Option<ItemId>`
  - `ConstantDef.canonical_path`
  - `ConstantRef.canonical_path`
  - `ConstantDef.stable_name`

Expected canonical paths:

```text
pkg::driver_a::DriverA::DEFAULT_SPEED
pkg::driver_b::DriverB::DEFAULT_SPEED
```

Expected stable names:

```text
DRIVER_A_DEFAULT_SPEED
DRIVER_B_DEFAULT_SPEED
```

Verdict:
- resolved cleanly

### 4. Identical short field names on different structs

Test shape:

```text
struct DriverA { regs: Mmio<...> }
struct DriverB { regs: Mmio<...> }
```

Evaluation:
- short field names collide
- field-level identity is resolved through:
  - `StructFieldDef.canonical_path`
  - `CrossLinkTable.path_to_field`

Expected canonical paths:

```text
pkg::driver_a::DriverA::regs
pkg::driver_b::DriverB::regs
```

Verdict:
- resolved cleanly
- this depends on the recently added field-level canonical path and field-level
  lookup table entry

### 5. Helper methods on non-driver helper types

Test shape:

```text
struct RxHelper { ... }

impl RxHelper {
    fn drain_one(&mut self) -> Option<u8> { ... }
}
```

Evaluation:
- the model does not assume that all inherent methods belong to the driver type
- `FunctionKind::InherentMethod { owner: ItemId }` is general enough
- `ImplDef.self_type` is general enough
- only ISR role is restricted to driver-owned inherent methods

Verdict:
- resolved cleanly
- this is a good sign that the ownership model is not overfit to driver-only
  methods

### 6. Multi-module-qualified names

Test shape:

```text
pkg::driver::Pl011FullSpec::init
pkg::support::timing::DelayHelper::from_us
```

Evaluation:
- first implementation slice keeps modules flat
- but identity uses `CanonicalPath`, which is fully qualified
- so canonical identity already scales to nested modules even though the
  first populated `Module` tree is intentionally simple

Verdict:
- logically resolved
- physical population of recursive module nesting is deferred, but the identity
  design does not block it

## Stress-Test Findings

### What the model handles well

The current design resolves all required ambiguity cases without reverting to:

- short-name matching
- backend heuristics
- implicit ownership inference

The strongest enablers are:

- `CanonicalPath`
- `ItemId`
- first-class `ImplDef`
- field-level canonical paths
- `CrossLinkTable`
- separation of `FunctionKind` and `FunctionRole`

### Minor consistency note

Section 3.2 of the research plan shows a shortened `FunctionDef` snippet that
omits `canonical_path`, while the earlier item-hierarchy definition includes it.

This is not an architectural gap, because the main type definition already
includes `canonical_path`, but it is a document-level inconsistency worth
cleaning up later for readability.

### No new design blocker found

This step did not surface a new architectural blocker.

The ownership and identity model is holding up well under the required stress
cases.

## Step 3 Verdict

Step 3 is **completed**.

Outcome:

- ownership and identity resolution are strong
- the design handles overlapping names and non-driver ownership correctly
- no new major design gap was discovered
