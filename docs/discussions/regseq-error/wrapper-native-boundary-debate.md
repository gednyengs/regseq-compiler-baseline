# Wrapper Native-Failure Boundary Debate

## Purpose

This note isolates one remaining `RegseqError` design question:

- may Regseq facade wrappers convert backend-native failures into semantic
  `RegseqResult` values visible to authored DSL code?
- or must backend-native failures stay outside wrapper-returned semantic flow?

This is a debate note, not a final resolution.

## Current Working Context

The current discussion has already narrowed the broader error model:

- `RegseqError` is a semantic DSL-level error surface
- helper functions are semantic-flow zones
- direct authored invocation of Regseq trait callback methods is heading toward
  rejection
- authored DSL commit points for semantic `RegseqError` currently appear to be
  fallible Regseq trait callback exits
- compiler-generated/backend-generated failures are a separate class of
  backend-native failures

That leaves the wrapper boundary as the main unresolved issue.

## Terms

### Semantic Regseq error

A `RegseqError` or `RegseqResult<T>` value used inside authored DSL code.

### Backend-native failure

A target-native failure outcome produced by generated or lowered OS/framework
 mechanisms.

Examples:

- Linux errno-like failure from a kernel API
- Zephyr errno-like failure from a kernel API
- framework-specific status outcomes on other targets

### Wrapper-mediated failure

A failure surfaced through a Regseq facade wrapper method such as:

```rust
self.clock.enable()?;
```

The wrapper may or may not hide a backend-native call underneath.

## The Core Question

When authored DSL code calls a fallible wrapper method, what is allowed to
happen?

## Model A: Wrappers May Collapse Backend-Native Failure Into Semantic Regseq Flow

Example:

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

Then authored DSL can write:

```rust
self.clock.enable()?;
```

or

```rust
match self.clock.enable() {
    RegseqResult::Ok(()) => {}
    RegseqResult::Err(RegseqError::Busy) => recover_or_retry(),
    RegseqResult::Err(e) => return RegseqResult::Err(e),
}
```

### Consequences

- wrapper methods participate fully in authored semantic Regseq error flow
- native OS/framework failures may be re-expressed as portable semantic errors
- the caller sees only semantic Regseq failure, not native origin
- the compiler may need wrapper-body analysis or wrapper-family rules to
  preserve provenance

### Upsides

- simple authored surface
- ergonomic wrapper APIs
- portable authored error handling over wrapper calls

### Risks

- backend-native distinctions may be collapsed too early
- some wrappers may expose semantic categories that are artificial for a target
- wrapper bodies become semantically significant translation boundaries

## Model B: Backend-Native Failure Must Stay Outside Wrapper-Returned Semantic Flow

Under this model, wrappers may not convert native backend failures into
 `RegseqResult`.

A wrapper like this is not allowed:

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

Instead, either:

1. the wrapper is purely semantic and does not hide backend-native failure
2. the native failure remains in compiler-generated/backend-generated code

Example of a semantic-only wrapper:

```rust
impl Clock {
    fn validate_expected_rate(&self) -> RegseqResult<()> {
        if self.expected_rate == 0 {
            return RegseqResult::Err(RegseqError::InvalidArgument);
        }
        RegseqResult::Ok(())
    }
}
```

Example of backend-native resource handling outside authored wrapper flow:

```rust
impl Lifecycle for Pl011 {
    fn init(&mut self) -> RegseqResult<()> {
        validate_clock_requirements(&self.clock)?;
        RegseqResult::Ok(())
    }
}
```

with generated backend code around the callback doing the actual target-native
 clock enable and propagating any native failure directly.

### Consequences

- authored DSL wrappers stay purely semantic
- backend-native failure remains in compiler-generated/backend-generated zones
- authored code cannot pattern-match on semantic wrapper failure that really
  came from native OS/framework calls

### Upsides

- sharp separation between semantic DSL failure and native backend failure
- less risk of collapsing target-specific distinctions too early
- simpler provenance story

### Risks

- wrapper APIs may feel weaker or less intuitive to spec authors
- some seemingly natural authored operations cannot expose failure through the
  wrapper surface
- compiler-generated preambles may need to own more operational behavior

## Model C: Hybrid Per-Wrapper Policy

Under this model:

- some wrappers may map backend-native failure into `RegseqResult`
- some wrappers may not

Possible examples:

- `poll::until(...)` may stay semantic
- `Clock::enable()` may stay backend-native
- `Completion::wait(timeout)` might or might not be allowed, depending on the
  construct contract

### Consequences

- more expressive
- but requires explicit per-wrapper policy
- and likely more compiler validation and documentation

### Risks

- inconsistent authored mental model
- more burden on extraction/analysis/lowering documentation

## Concrete Examples To Judge

### Clock enable

```rust
self.clock.enable()?;
```

Should authored DSL be able to handle `Busy` here as a semantic Regseq error?

### Reset deassert

```rust
self.reset.deassert()?;
```

Should deassert failure be semantic, native, or policy-dependent?

### Poll until

```rust
regseq::poll::until(
    || self.regs.fr().txff().read() == 0,
    Duration::from_us(10),
    Duration::from_ms(100),
)?;
```

This already looks closer to a semantic timeout operation. Does that argue for
 wrapper-level semantic error more generally, or only for some wrappers?

### Completion wait

```rust
self.tx_done.wait(Duration::from_ms(100))?;
```

Is this semantic timeout/busy behavior, or native synchronization failure?

## What A Good Recommendation Should Cover

Any recommendation should answer:

1. Which model is preferred: A, B, or C?
2. Why is that model best for authored DSL semantics?
3. How does it interact with backend-native failures on your target family?
4. What should the compiler preserve for extraction and analysis?
5. What authored syntax becomes valid or invalid under that choice?
6. If hybrid, what rule decides which wrappers may translate native failure?

## Current Lean

The current discussion has not resolved this.

The likely stable common ground so far is:

- helper functions remain semantic-flow zones
- trait callback exits are authored commit points
- backend-generated failures remain backend-native unless explicitly translated

The wrapper boundary is the remaining unresolved case.
