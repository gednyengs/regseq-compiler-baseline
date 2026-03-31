# RegseqError Flow Scenarios

## Purpose

These scenarios are meant to validate a specific model:

- `RegseqError` can flow as an abstract semantic value inside authored Regseq
  code
- backend-specific error materialization should happen only at explicit lowering
  commit points

This document is not a final design. It is a test set for discussion.

## Assumed Surface

The examples below assume:

```rust
enum RegseqError {
    InvalidArgument,
    Busy,
    Timeout,
    IoError,
}

enum RegseqResult<T> {
    Ok(T),
    Err(RegseqError),
}
```

The examples are meant to exercise flow shape, not settle the final API.

## Scenario 1: Helper Creates an Error Locally

```rust
fn validate_baud(baud: u32) -> RegseqResult<()> {
    if baud == 0 {
        return RegseqResult::Err(RegseqError::InvalidArgument);
    }
    RegseqResult::Ok(())
}
```

### Why this matters

This is the simplest authored semantic failure site.

### Expected interpretation under the model

- the compiler should treat the `RegseqError` as an abstract semantic value
- no backend-specific error mechanism should be committed yet
- this function body is still ordinary authored DSL flow

### What this validates

- creating a `RegseqError` is not itself a lowering commit point

## Scenario 2: Helper Propagates Another Helper With `?`

```rust
fn validate_line_settings(baud: u32, stop_bits: u8) -> RegseqResult<()> {
    validate_baud(baud)?;
    if stop_bits == 0 {
        return RegseqResult::Err(RegseqError::InvalidArgument);
    }
    RegseqResult::Ok(())
}
```

### Why this matters

This distinguishes semantic propagation from backend propagation.

### Expected interpretation under the model

- `?` here should still stay inside semantic Regseq flow
- no OS-specific error mechanism should be materialized yet
- the compiler should see this as `RegseqResult` propagation inside helper code

### What this validates

- `?` is not automatically a lowering commit point
- `?` only becomes lowering-relevant when it escapes across a backend-facing
  boundary

## Scenario 3: Helper Handles the Error Locally

```rust
fn maybe_recover_clock(clock: &Clock) -> RegseqResult<()> {
    match clock.enable() {
        RegseqResult::Ok(()) => RegseqResult::Ok(()),
        RegseqResult::Err(RegseqError::Busy) => RegseqResult::Ok(()),
        RegseqResult::Err(e) => RegseqResult::Err(e),
    }
}
```

### Why this matters

This is the clearest case where the author owns the branching logic.

### Expected interpretation under the model

- local pattern matching should be treated as authored semantic control flow
- the compiler should not force backend-specific error handling just because
  `RegseqError` is being matched
- only the final returned `RegseqResult` remains relevant to any later commit
  point

### What this validates

- matching and inspecting `RegseqError` stays inside the authored semantic world

## Scenario 4: Helper Normalizes or Reclassifies Errors

```rust
fn enable_clock_with_timeout(clock: &Clock) -> RegseqResult<()> {
    match clock.enable() {
        RegseqResult::Ok(()) => RegseqResult::Ok(()),
        RegseqResult::Err(RegseqError::Busy) => RegseqResult::Err(RegseqError::Timeout),
        RegseqResult::Err(e) => RegseqResult::Err(e),
    }
}
```

### Why this matters

This tests whether helper functions are allowed to reinterpret semantic errors
before they reach a backend-facing boundary.

### Expected interpretation under the model

- this is still semantic Regseq flow
- reclassification should be allowed at the DSL level
- no backend-native mechanism should be committed yet

### What this validates

- helper functions may transform semantic Regseq errors before commit

## Scenario 5: Lifecycle Boundary Commits the Result

```rust
impl Lifecycle for Pl011 {
    fn init(&mut self) -> RegseqResult<()> {
        validate_line_settings(self.baud, self.stop_bits)?;
        enable_clock_with_timeout(&self.clock)?;
        RegseqResult::Ok(())
    }
}
```

### Why this matters

This is the first obvious candidate for a lowering commit point.

### Expected interpretation under the model

- inside the body, `RegseqResult` still flows semantically
- when `init` exits, the compiler must lower the final success/error outcome into
  the backend-specific callback result mechanism
- this is where authored semantic errors stop being abstract

### What this validates

- trait callback returns are commit points

## Scenario 6: Wrapper Maps Native Failure Into Semantic Regseq Flow

```rust
impl Clock {
    fn enable(&self) -> RegseqResult<()> {
        let status = self.enable_native();
        if status != 0 {
            return RegseqResult::Err(RegseqError::Busy);
        }
        RegseqResult::Ok(())
    }
}
```

### Why this matters

This is the wrapper-boundary case we have been debating.

### Expected interpretation under the model

There are three candidate interpretations:

1. allowed semantic collapse
   - wrapper converts backend-native failure into Regseq semantic failure
2. forbidden collapse
   - wrapper must not convert native failure into `RegseqResult`
3. policy-driven collapse
   - allowed for some wrappers, forbidden for others

### What this validates

- whether wrappers are inside or outside the semantic Regseq error world

## Scenario 7: Compiler-Generated Resource Setup Failure

```rust
impl Lifecycle for Pl011 {
    fn init(&mut self) -> RegseqResult<()> {
        RegseqResult::Ok(())
    }
}
```

Assume the backend generates resource setup around this body:

```c
ret = clk_prepare_enable(clk);
if (ret)
    return ret;
```

### Why this matters

This is the cleanest backend-native failure case.

### Expected interpretation under the model

- the authored DSL did not create a `RegseqError`
- the generated backend failure should remain backend-native
- no semantic `RegseqError` needs to be synthesized unless the design
  explicitly requires it

### What this validates

- compiler-generated native failures are not automatically semantic Regseq
  failures

## Scenario 8: Boundary With Explicit Semantic Error Return

```rust
impl Lifecycle for Pl011 {
    fn init(&mut self) -> RegseqResult<()> {
        if self.baud == 0 {
            return RegseqResult::Err(RegseqError::InvalidArgument);
        }
        RegseqResult::Ok(())
    }
}
```

### Why this matters

This is the simplest semantic error that must cross a lowering boundary.

### Expected interpretation under the model

- the body remains semantic Regseq logic
- the returned semantic error must be lowered when the callback exits

### What this validates

- explicit semantic error returns are commit-relevant only when they escape a
  backend-facing boundary

## Scenario 9: Nested Helpers Before a Commit Point

```rust
fn helper_a() -> RegseqResult<()> {
    RegseqResult::Err(RegseqError::Busy)
}

fn helper_b() -> RegseqResult<()> {
    helper_a()?;
    RegseqResult::Ok(())
}

fn helper_c() -> RegseqResult<()> {
    helper_b()?;
    RegseqResult::Ok(())
}
```

### Why this matters

This tests whether semantic Regseq flow can remain abstract across multiple
stack frames.

### Expected interpretation under the model

- none of these helper boundaries should force backend lowering on their own
- the chain should remain semantic until a real commit point is reached

### What this validates

- helper-to-helper propagation should not be confused with backend lowering

## Scenario 10: Commit Point Hidden Behind a Driver Helper

```rust
fn perform_init(driver: &mut Pl011) -> RegseqResult<()> {
    validate_line_settings(driver.baud, driver.stop_bits)?;
    driver.clock.enable()?;
    RegseqResult::Ok(())
}

impl Lifecycle for Pl011 {
    fn init(&mut self) -> RegseqResult<()> {
        perform_init(self)
    }
}
```

### Why this matters

This tests whether the compiler correctly identifies the real commit point as
the callback boundary, not the helper boundary.

### Expected interpretation under the model

- `perform_init` remains semantic flow
- the commit happens when `Lifecycle::init` returns its final value to backend
  glue

### What this validates

- commit points are semantic boundaries, not just any function that returns
  `RegseqResult`

## Questions These Scenarios Should Answer

1. Is `RegseqResult` allowed to stay abstract through helper stacks?
2. Are trait callback returns the main lowering commit points?
3. Are wrapper bodies allowed to collapse backend-native failures into semantic
   `RegseqResult`?
4. Is compiler-generated backend failure always outside the semantic Regseq
   error world?
5. Does local pattern matching belong entirely to authored semantic flow?

## Tentative Model If These Scenarios Hold

- helper functions are semantic-flow zones
- local branching and reclassification are semantic-flow operations
- trait callback exits are lowering commit points
- compiler-generated resource-management failures are backend-native unless
  explicitly translated
- wrapper failure policy remains the main unresolved issue
