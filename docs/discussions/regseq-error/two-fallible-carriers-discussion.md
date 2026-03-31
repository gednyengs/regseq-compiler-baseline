# Two Fallible Carriers Discussion

## Purpose

This note explores a refinement of the current `RegseqError` discussion.

The previous debate treated the wrapper/native boundary as if the choice were:

- semantic `RegseqResult` flow
- or no fallible carrier at all for backend-native operational failure

That framing is too weak because it ignores control-flow structure.

If an authored operation can fail and short-circuit, the compiler may still need
an explicit fallible carrier even when the failure is not part of the semantic
`RegseqError` world. In particular, a fallible carrier can encode:

- success path
- failure path
- early-exit structure
- drop elaboration for droppable resources on the failure path

This suggests a possible design with **two fallible carriers** rather than one.

## Core Observation

A result-like carrier is not only about the meaning of the error payload.

It also provides:

- structured branching
- explicit propagation sites
- clear MIR lowering through `Try`-like machinery
- a path where resource cleanup/drop elaboration can happen correctly

So even if an operation's failure is backend-native rather than semantic, a
first-class fallible carrier may still be necessary in authored DSL code.

## Candidate Split

### 1. Semantic fallible carrier

This is the current `RegseqError` world.

Example shape:

```rust
enum RegseqError {
    InvalidArgument,
    Timeout,
    Busy,
    IoError,
}

enum RegseqResult<T> {
    Ok(T),
    Err(RegseqError),
}
```

Intended meaning:

- author may inspect and reason about the failure semantically
- author may match, transform, and propagate it
- this is the portable DSL-level failure surface

Examples:

- `poll::until(...) -> RegseqResult<()>`
- helper functions returning `RegseqResult<_>`
- trait callback exits returning `RegseqResult<_>`

### 2. Backend-native fallible carrier

This is a new possible category.

Example shape:

```rust
enum NativeFailure {
    // intentionally opaque or narrowly structured
}

enum NativeResult<T> {
    Ok(T),
    Err(NativeFailure),
}
```

The actual syntax is open. The important point is the semantic role:

- this carrier is still fallible
- it still supports structured early-exit
- but its failure is not part of the semantic `RegseqError` model
- its main job is to carry backend-native operational failure through authored
  control flow when necessary

## Why This Came Up

The earlier wrapper-boundary debate made it seem like backend-native operational
failure had only two choices:

1. map into `RegseqResult`
2. stay entirely outside authored flow

That misses a third possibility:

3. stay fallible and structured, but not semantic `RegseqError`

This matters because operations like:

```rust
self.clock.enable()?;
```

may need:

- early-exit
- caller-visible control-flow behavior
- drop elaboration on failure

even if the failure should not be semantically exposed as `RegseqError::Busy`
or `RegseqError::IoError`.

## How This Could Help

### A. It preserves control-flow structure

If wrappers or intrinsics need to fail in authored code, a non-semantic
fallible carrier still gives the compiler a structured failure path.

### B. It avoids false semantic promises

If a target treats `Clock::enable()` as a no-op, or if native failure meanings
are too target-specific, the DSL does not have to pretend the failure is a
portable semantic `RegseqError`.

### C. It may reduce pressure on the wrapper-boundary debate

Instead of forcing every wrapper into one of:

- semantic `RegseqResult`
- not fallible in authored DSL

we could allow a middle category:

- authored fallible with backend-native failure payload

## Example Scenarios

### Scenario A: Semantic timeout operation

```rust
regseq::poll::until(
    || self.regs.fr().txff().read() == 0,
    Duration::from_us(10),
    Duration::from_ms(100),
)?;
```

This still looks like a natural `RegseqResult<()>` operation.

Its failure (`Timeout`) is part of the authored contract itself.

### Scenario B: Backend-native operational wrapper

```rust
self.clock.enable()?;
```

Possible interpretation under the two-carrier model:

- `clock.enable()` returns a backend-native fallible carrier, not
  `RegseqResult`
- it still supports `?` and early-exit
- failure is not inspectable as portable semantic `RegseqError`
- drop elaboration still works correctly on the failure path

### Scenario C: Mixed helper body

```rust
fn helper(&mut self) -> ??? {
    self.clock.enable()?;
    regseq::poll::until(...)?;
    RegseqResult::Ok(())
}
```

This immediately raises an interop question:

- can one function propagate both carriers?
- if so, how?
- if not, how should such code be written?

This is one of the biggest design tensions in the proposal.

## Main Open Questions

### 1. Does the DSL really want two propagated fallible carriers?

This is the primary question.

A second fallible carrier may solve some backend-native boundary problems, but
it could also make authored code much harder to reason about.

### 2. What is the host-language surface?

Possible options:

- `RegseqResult<T>` and `NativeResult<T>` as separate custom carriers
- one generic carrier with two error domains
- an opaque backend-native carrier disallowed from local inspection

### 3. How does `?` work across both carriers?

If the DSL uses two distinct carriers, we must decide:

- whether both support `?`
- whether one can convert into the other
- whether a function may propagate both
- whether interconversion is implicit or explicit

### 4. Can authors inspect backend-native failure?

This is critical.

If authors can match on the backend-native carrier, then the DSL starts
surfacing target-sensitive failure semantics directly.

If authors cannot inspect it, then what operations are allowed?

### 5. Does this improve or worsen portability?

The proposal may preserve correctness for target-specific failures, but it may
also make the DSL less portable or less ergonomic.

### 6. Does this make wrapper policy simpler or more complex?

This is a direct test against the earlier debate.

If two carriers exist, then wrappers might be classified as:

- semantic wrappers returning `RegseqResult`
- operational wrappers returning backend-native fallible carrier

The question is whether this is cleaner than Model C's per-wrapper policy, or
just a more complicated version of it.

## Potential Benefits

- preserves structured failure paths where drop elaboration matters
- avoids overloading `RegseqError` with backend-native meaning
- may better fit targets where some operational failures are real and others are
  no-op

## Potential Risks

- two fallible carriers may make authored DSL much harder to learn
- `?` propagation rules may become complicated
- helper function signatures may become messy
- extraction/analysis may need to model two separate early-exit worlds
- some functions may need awkward carrier conversion or bridging rules

## Questions For Evaluation

Any recommendation on this proposal should answer:

1. Does introducing a second fallible carrier solve the main problem from your
   earlier wrapper-boundary position?
2. Does it preserve the benefits of your earlier recommendation?
3. What new complexity does it introduce for authored DSL code?
4. Would your target family actually benefit from this, or is it unnecessary?
5. If adopted, which operations would belong in the semantic carrier and which
   would belong in the backend-native fallible carrier?

## Current Status

This is a discussion-only proposal.

No decision has been made yet on:

- whether a second fallible carrier should exist
- whether it should be inspectable by authored DSL code
- whether it should propagate with `?`
- how it would interact with `RegseqResult`
