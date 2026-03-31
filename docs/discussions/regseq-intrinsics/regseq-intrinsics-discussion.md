# Regseq Intrinsics Discussion

## Purpose

This note captures the current discussion about the shape and compiler role of
Regseq intrinsics. It is intentionally self-contained and should serve as the
working discussion base before any intrinsic-specific tasking or implementation
mechanics are finalized.

## Current Direction

The current preferred direction is:

1. canonical Regseq intrinsics live under a dedicated namespace:
   - `regseq::intrinsics::construct::operation(...)`
2. intrinsics are compiler-known operations, not user-extensible functions
3. the compiler identifies supported intrinsics by known paths
4. `#[rseq::intrinsic]` is a declaration/consistency marker, not an extension
   mechanism
5. facade macros and wrappers may provide ergonomics, but canonical lowering
   semantics belong to the intrinsic paths

## Preferred Surface Shape

Preferred canonical form:

```rust
regseq::intrinsics::construct::operation(args...)
```

Examples:

```rust
regseq::intrinsics::sync::memory_barrier(BarrierKind::WriteWrite);
regseq::intrinsics::dma::sync_for_device(&buffer);
```

This gives the compiler one explicit namespace for function-like operations that
it must lower specially.

Decision captured here:

- all function-like Regseq intrinsics must have their canonical home under
  `regseq::intrinsics::{construct}::{intrinsic}`
- other public paths may exist as aliases, re-exports, wrappers, or convenience
  sugar
- those alternate paths are not the canonical intrinsic-definition location
- existing accepted function-like intrinsic surfaces outside this namespace
  should be treated as compatibility aliases or re-exports once the canonical
  intrinsic-home path exists

## Intrinsic Declaration Pattern

The current declaration pattern under discussion is:

```rust
#[inline(never)]
#[rseq::intrinsic]
pub fn ensure_visibility() {
    unimplemented!()
}
```

The intended meaning is:

- the facade declares the intrinsic surface in ordinary Rust
- the Rust compiler typechecks that declaration like normal Rust
- the Regseq compiler recognizes the intrinsic by its known path
- the Regseq compiler does not treat the function body as the semantic
  implementation of the intrinsic
- intrinsic declarations should be marked `#[inline(never)]` so the host Rust
  compiler does not inline them away before Regseq compiler infrastructure has a
  stable call surface to recognize

Decision captured here:

- function-like intrinsic declarations should carry `#[inline(never)]`
- this is part of the expected declaration pattern, not an optional style hint

## Role Of Intrinsic Bodies

The current model distinguishes two cases:

1. bodyless-in-practice intrinsic stubs
   - the intrinsic body is just `unimplemented!()` or equivalent
   - the point is to provide a typechecked call surface in Rust

2. constraint-enforcing intrinsic declarations
   - a body may be present if it helps enforce interface constraints through
     ordinary Rust typing or expression structure
   - even then, the Regseq compiler should ignore the body as semantic
     implementation logic

So the body may help Rust check the interface, but it should not define the
Regseq meaning.

## Intrinsic Identity

The current preference is:

- intrinsic identity comes from compiler-known paths
- `#[rseq::intrinsic]` is required for consistency
- function-like intrinsic declarations should also carry `#[inline(never)]`
- the attribute alone must not allow users to invent new compiler intrinsics

This implies the following validation rules:

1. known intrinsic path + `#[rseq::intrinsic]`
   - accepted

2. known intrinsic path without `#[rseq::intrinsic]`
   - compiler should reject or flag as a facade/compiler mismatch

3. `#[rseq::intrinsic]` on an unknown function path
   - compiler should reject

This preserves the fact that Regseq intrinsics are a closed, compiler-owned set.

Decision captured here:

- intrinsic validity requires both:
  - the compiler-known canonical intrinsic path
  - the `#[rseq::intrinsic]` marker on the declaration
- path identity is the semantic identity
- the attribute is required for consistency, validation, and future tooling

## Relationship To Macros And Wrappers

The current preferred layering is:

1. canonical intrinsic operations
   - live under `regseq::intrinsics::...`
   - compiler lowers these directly

2. facade wrappers
   - may provide ergonomic method or helper surfaces
   - may call canonical intrinsics

3. facade macros
   - may expand into ordinary authored code plus canonical intrinsic calls
   - are syntax sugar, not the canonical lowering anchors

Under this model, a macro like `regseq::ordered!` is best understood as a
surface-level convenience that expands into explicit intrinsic operations rather
than as the primitive itself.

Additional decisions captured here:

- wrapper methods on typed resources are not function-like intrinsics
- they are a distinct wrapper-operation family that may internally lower to
  compiler-known behavior without becoming canonical intrinsic declarations
- macro-form surfaces are not canonical function-like intrinsics
- macro-form surfaces are syntax sugar that expand into ordinary authored code,
  canonical intrinsic calls, or both

## Why This Direction Is Attractive

This direction provides:

- a single explicit namespace for compiler-known operations
- less ad hoc path recognition across the facade
- a clearer separation between:
  - canonical operations
  - wrapper ergonomics
  - macro sugar
- a better validation story for compiler/facade consistency

## Clarifications And Carry-Forwards

The main discussion questions are now refined enough to carry as explicit
working decisions:

1. Existing accepted function-like intrinsic surfaces
   - if a previously accepted intrinsic path exists outside
     `regseq::intrinsics::...`, it should be treated as an alias, re-export, or
     compatibility path
   - the canonical intrinsic-definition home still belongs under
     `regseq::intrinsics::{construct}::{intrinsic}`

2. Function-like intrinsic declaration requirements
   - require the canonical known path
   - require `#[rseq::intrinsic]`
   - require `#[inline(never)]`

3. Declaration-side intrinsic metadata
   - should stay minimal
   - should not become a single fixed schema for all compiler analyses
   - analysis-specific facts are interpreted by the compiler per pass

4. Wrapper/macro boundary
   - wrapper methods remain wrapper operations, not canonical function-like
     intrinsics
   - macro surfaces remain syntax sugar, not canonical function-like intrinsics
   - canonical lowering identity stays on the function-like intrinsic path

5. Macro predictability
   - macro sugar such as `regseq::ordered!` should expand into a predictable
     combination of authored code and canonical intrinsic calls
   - the compiler should not need a separate semantic identity for the macro
     beyond its expansion contract

The remaining work is therefore not to settle the high-level intrinsic model,
but to apply it consistently when authoring task specs and later facade
surfaces.

## Current Working Summary

The current working model is:

- Regseq intrinsics are compiler-owned, closed-set operations
- their canonical surface is a dedicated namespace under
  `regseq::intrinsics::...`
- all function-like intrinsics must live there canonically, even if re-exported
  elsewhere for convenience
- the compiler recognizes them by known paths
- `#[rseq::intrinsic]` is required as a consistency marker and tooling hook
- `#[inline(never)]` is part of the expected declaration pattern for
  function-like intrinsic declarations
- declaration-side intrinsic metadata should stay minimal; analysis-specific
  facts are interpreted by the compiler according to the pass being performed
- existing accepted function-like intrinsic paths outside
  `regseq::intrinsics::...` are compatibility aliases or re-exports, not the
  canonical intrinsic-definition home
- facade wrappers are allowed, but they are not canonical function-like
  intrinsics
- facade macros are allowed, but they are syntax sugar rather than canonical
  lowering anchors

## Status

This discussion is now sufficiently refined to support task-spec decomposition.
The remaining work should happen in task design and later construct-specific
notes rather than by reopening the high-level intrinsic model.
