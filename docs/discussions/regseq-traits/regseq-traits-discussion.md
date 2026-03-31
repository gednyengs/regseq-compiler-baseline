# Regseq Traits Discussion

## Purpose

This note discusses the role of Regseq traits in the DSL and compiler model.

The main question is whether Regseq traits should be treated as ordinary
callable Rust abstraction surfaces, or whether they should be restricted to a
single purpose: defining callback/glue contracts that the compiler lowers into
OS-specific mechanisms.

This document is a discussion note, not a final resolution.

## Current Direction

The current direction is:

- Regseq traits exist only to capture callback/glue logic
- Regseq trait methods are implemented by the driver author
- Regseq trait methods are invoked by compiler-generated or backend-generated
  glue
- Regseq trait methods are not part of the ordinary reusable authored API
  surface

Under this model, Regseq traits are not a general-purpose language mechanism
for arbitrary behavior sharing.

## Why This Matters

If Regseq trait methods are treated like ordinary callable methods, several
important boundaries become unclear:

- callback entrypoints versus reusable helpers
- authored semantic flow versus backend entry surfaces
- trait callbacks versus wrapper operations
- trait callbacks versus intrinsics

That confusion would make it harder to:

- define lowering commit points clearly
- explain callback ownership to spec authors
- validate authored DSL usage
- decide where RegseqError handling should commit to backend-native outcomes

## Proposed Rule

Regseq traits should be treated as callback contracts only.

That means:

- the driver author may implement Regseq traits
- the compiler/backend may lower those implementations into OS/framework
  callback mechanisms
- authored Regseq code must not directly invoke Regseq trait methods

Examples of calls that should be rejected in authored DSL:

```rust
self.init()?;
Lifecycle::init(self)?;
Serial::port_open(self)?;
```

These are callback-entry surfaces, not ordinary helper APIs.

## What Should Be Used Instead

If authored Regseq code needs reusable logic, it should use one of the
following:

### 1. Helper functions

Use ordinary helper functions for shared local logic:

```rust
fn perform_common_init(driver: &mut Pl011) -> RegseqResult<()> {
    // shared semantic logic
    RegseqResult::Ok(())
}

impl Lifecycle for Pl011 {
    fn init(&mut self) -> RegseqResult<()> {
        perform_common_init(self)
    }
}
```

### 2. Intrinsics

Use intrinsics for explicit special operations that the compiler must recognize
and lower.

### 3. Wrapper methods

Use typed-resource wrapper methods for reusable resource-local operations.

### 4. Wrapper methods over intrinsics

Use wrapper methods as ergonomic authored surfaces over underlying intrinsic
operations where that is the accepted design.

## Why Query-Style Methods Do Not Belong on Traits

If a query or utility operation is harmless and reusable, that is an argument
against putting it on a Regseq trait.

Such behavior should instead live on:

- wrapper/resource types
- intrinsics
- helper functions

This keeps Regseq traits narrowly focused on the callback contracts that the
compiler must materialize into OS-specific glue.

## Relationship To Error Handling

This boundary helps clarify RegseqError handling.

If Regseq trait methods are callback entrypoints only, then:

- trait method exits are natural lowering commit points
- helper functions remain semantic-flow zones
- wrapper/intrinsic surfaces can be discussed separately from callback entry
  semantics

This makes it easier to reason about:

- when `RegseqResult` remains abstract
- when authored semantic failure commits into backend-native error handling

## Frontend Validation Implication

If this direction is accepted, the Regseq compiler should diagnose direct
authored invocation of Regseq trait methods.

Suggested intent of the rule:

- allowed: implementing a Regseq trait
- allowed: compiler-generated/backend-generated invocation of the trait method
- rejected: direct authored invocation from DSL code

The diagnostic should likely suggest moving reusable logic into a helper
function, intrinsic, or wrapper method.

## What This Would Mean For Phase B

If accepted, this rule affects at least:

- trait-capture tasks
- frontend validation tasks
- RegseqError boundary discussions
- callback commit-point modeling

It should be resolved before the trait-capture task specs become too concrete.

## Open Questions

1. Should the prohibition cover only direct calls, or also taking references to
   trait methods?
2. Should the rule apply uniformly to all Regseq callback traits?
3. Should the compiler detect this at HIR extraction time or in a dedicated
   validation pass?
4. Should diagnostics mention the replacement mechanisms explicitly:
   helper function, intrinsic, wrapper method?

## Tentative Position

The current tentative position is:

- Regseq traits are callback/glue contracts only
- they are not general callable abstraction surfaces
- direct authored invocation should be rejected
- reusable/query behavior should be surfaced elsewhere
