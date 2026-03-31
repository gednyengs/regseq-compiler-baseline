# Perspective 5: Extraction and Analysis Signal Quality

## Position Statement

The compiler's extraction pipeline must recognize authored `RegseqError` returns as **first-class semantic signals**, not as secondary implementation noise. Every authored error site should produce a clear extraction artifact that captures:

1. **The error category** (what semantic failure happened)
2. **The construct context** (which construct/operation threw it)
3. **The control-flow consequence** (early return, error propagation, or error handling)

This precision matters because multiple analysis passes depend on error-flow information:
- **CFG-based reachability** needs to know which paths are error paths
- **Resource cleanup verification** needs error sites to determine finalization order
- **Lowering validation** needs semantic categories to pick target-native error codes
- **Backend code generation** needs error flow to emit correct error paths in C

Treating error information as vague would weaken all of these.

---

## 1. What the Compiler Extractor Must See

### A. Direct Enum Variant Construction

When a spec author writes:

```rust
impl Lifecycle for Pl011 {
    fn init(&mut self) -> Result<(), RegseqError> {
        // Some setup...
        self.clock.enable()
            .map_err(|_| RegseqError::ResourceUnavailable)?;
        // More setup...
        Ok(())
    }
}
```

The extractor should recognize:

- **Site**: `clock.enable()` at line N inside `Lifecycle::init`
- **Operation**: clock management wrapper, tagged as `acquire_clock` semantically
- **Error category**: The author's intent is "I cannot acquire this resource"
- **Control flow**: Early return via `?` operator, caller receives `RegseqError`

This produces an extraction artifact:

```
error_site {
  location: Lifecycle::init, line N
  construct: clock_management.enable
  category: ResourceUnavailable
  propagates_to: caller_result
}
```

### B. Explicit Error Construction

If the author constructs a specific error variant:

```rust
if self.dma.channel.is_none() {
    return Err(RegseqError::ConfigurationMissing);
}
```

The extractor should see:

- **Direct construction** of the `ConfigurationMissing` variant
- **Control-flow branch** (conditional check + early return)
- **Semantic intent**: "The expected configuration is not present"

Artifact:
```
error_site {
  location: Lifecycle::init, line N
  category: ConfigurationMissing
  authored: true
  propagates_to: caller_result
}
```

### C. Wrapper-Method Error Propagation

When an authored method calls a wrapper and propagates its error:

```rust
fn configure_serial(&mut self) -> Result<(), RegseqError> {
    self.serial.port_open()?;  // Propagates RegseqError::*
    self.serial.configure(&config)?;  // Propagates RegseqError::*
    Ok(())
}
```

The extractor should track that:

- Both wrapper calls are **fallible** (return type is `Result`)
- Both use the **`?` operator**, creating implicit error propagation
- The caller's result type is also `Result<(), RegseqError>`

Artifact per call:
```
error_site {
  location: configure_serial, line N
  construct: serial.port_open
  fallible: true
  propagates_via: question_mark_operator
  propagates_to: caller_result
}
```

---

## 2. Should the Compiler Extract Specific Categories or Just "Fallible"?

**Answer: Extract both, but for different purposes.**

### Specific Categories Enable Fine-Grained Lowering

When the backend lowers this error to C:

```rust
RegseqError::ResourceUnavailable
```

The backend chooses the target-native error code:

```c
// Linux: ENODEV (19) or EPROBE_DEFER (-517)
// Zephyr: -ENODEV or -EAGAIN
// Custom OS: vendor-specific code
```

If the extractor only records "some error happened", the backend has no semantic guidance. It cannot distinguish between "resource truly unavailable" and "transient defer-able availability".

### Generic Fallibility Enables CFG-Based Analysis

For control-flow reachability and resource cleanup ordering, it's sufficient to know:

- "This operation can fail"
- "On failure, control returns to caller"
- "On failure, cleanup code at [X] still runs"

The specific error variant is less important for these analyses.

**Recommendation**: Extract and carry both levels:

1. **Specific category** (for lowering and diagnostics)
2. **Generic fallibility** (for CFG analysis and cleanup ordering)

In the extraction artifact:

```
error_site {
  location: Lifecycle::init, line N
  construct: clock.enable
  fallible: true
  category: ResourceUnavailable
  propagates_to: caller_result
}
```

The analysis pass that needs specific semantics uses the `category` field. The CFG pass uses `fallible: true`.

---

## 3. Error Extraction and the `?` Operator

### The `?` Operator as an Extraction Signal

The `?` operator is a **visual marker** that says: "If this operation fails, I want the error to propagate to my caller."

When the extractor sees:

```rust
self.clock.enable()?;
```

it should record:

- **Explicit error propagation** (not hidden in a wrapper)
- **Semantic intent**: Author chose to let the caller handle this error
- **Result type**: The enclosing function's return type determines how this propagates

### Early Returns Without `?`

Some authors will write:

```rust
let result = self.clock.enable();
if let Err(e) = result {
    log_error("Clock enable failed");
    return Err(e);
}
```

This is **explicit error propagation** without the `?` operator. The extraction artifact should still capture:

```
error_site {
  location: Lifecycle::init, line N
  construct: clock.enable
  fallible: true
  category: ResourceUnavailable
  propagates_to: caller_result
  propagation_style: explicit_match
}
```

### Error Handling (Non-Propagation)

Some code will handle the error:

```rust
let result = self.clock.enable();
if let Err(_) = result {
    // Try the fallback clock domain
    self.clock.enable_fallback()?;
}
```

Here, the first `clock.enable()` **does not propagate** to the caller. The extraction artifact reflects this:

```
error_site {
  location: Lifecycle::init, line N
  construct: clock.enable
  fallible: true
  category: ResourceUnavailable
  propagates_to: internal_handling  // Not propagated to caller
}
```

This distinction matters for cleanup analysis: if the error is handled and doesn't propagate, the cleanup code doesn't need to execute an "error path", only the "success path".

---

## 4. Should the Compiler Verify Error Categories Are Semantically Appropriate?

**Yes, with caveats.**

### Validation Opportunities

For some constructs, the set of reasonable error categories is small and knowable:

- **Clock enable** should only return `ResourceUnavailable`, never `InvalidParameter` (the parameter is compile-time)
- **Serial port open** might return `ResourceUnavailable` (port in use) or `ConfigurationMissing` (no hardware binding)
- **Register poll** might return `Timeout` or `DeviceNotResponding`, but not `ConfigurationMissing`

The compiler could validate:

```
construct: clock.enable
allowed_error_categories: [ResourceUnavailable]
```

If the author returns a different category, emit a compiler diagnostic:

```
warning: clock.enable() returned ConfigurationMissing, but allowed categories are [ResourceUnavailable]
        Hint: If you're checking for missing clock binding, do this in Lifecycle::init before calling enable().
```

### When NOT to Validate

Some constructs have genuinely diverse error semantics:

- **User-written helper methods** might return any authored error
- **Opaque external code** (if ever integrated) might return categories the compiler doesn't know about

For those, validation should be **optional and configurable**, not enforced.

### Practical Rule

**Validate error categories for standard constructs (clock, reset, pin, register, DMA), suggest for user-written methods.**

---

## 5. Analysis Passes That Benefit From Error-Flow Information

### A. Resource Cleanup Verification

Example: "Did I properly release all acquired resources before returning an error?"

The analysis needs to know:

- Which operations acquired resources (`Clock::enable`, `RegisterAccess::map_region`, etc.)
- Which error sites can cause early returns
- Which cleanup code runs on the error path

```rust
fn init(&mut self) -> Result<(), RegseqError> {
    self.clock.enable()?;  // Acquires clock
    self.regs.map();       // Acquires MMIO region
    self.irq.register()?;  // Acquires interrupt handler

    // If any ?-error reaches here, all three resources must clean up
    // Compiler verifies this via error-flow + resource-ownership analysis

    self.init_hardware()?;
    Ok(())
}
```

The error-flow artifact enables the compiler to verify:
- Error from line 2 → clock cleanup needed
- Error from line 3 → clock + MMIO cleanup needed
- Error from line 4 → clock + MMIO + interrupt cleanup needed

Without error-flow information, the compiler can only assume "worst case, maybe all cleanups are needed" and lose precision.

### B. Control-Flow Graph (CFG) Reachability

The compiler can build an error-aware CFG:

```
BasicBlock::init {
  clock.enable()
    [success] -> next_block
    [error]   -> cleanup_block (propagates to caller)
}
```

This guides the backend to emit separate code paths for error vs. success.

### C. Error Path Lowering

For Linux goto-cleanup chains, the compiler needs to know:

- Which resources need cleanup on error
- In what order (LIFO acquisition order)
- Which error categories to map to which errno values

Example:

```c
// Generated C (pseudocode)
int pl011_init(struct pl011 *dev) {
    if ((err = pl011_enable_clock(dev)) != 0) {
        // Error happened before resources acquired
        return err;
    }
    if ((err = pl011_map_mmio(dev)) != 0) {
        // Error: clock acquired, must release
        pl011_disable_clock(dev);
        return err;
    }
    if ((err = pl011_register_irq(dev)) != 0) {
        // Error: clock + MMIO acquired, must release both
        pl011_unregister_irq(dev);
        pl011_unmap_mmio(dev);
        pl011_disable_clock(dev);
        return err;
    }
    return 0;
}
```

Generating this correctly **requires precise knowledge of which operations can fail** and **in what order resources are acquired**.

### D. Diagnostic Logging

The compiler could emit optional logging around error sites:

```c
if ((err = pl011_enable_clock(dev)) != 0) {
    rseq_log_error("PL011 init: clock enable failed with code %d", err);
    return err;
}
```

This is optional and backend-controlled, but it relies on the extraction having identified the error site.

---

## 6. Raw Extraction Artifact Representation

The extraction should produce a structured, machine-readable output.

### Example Artifact Structure

```json
{
  "construct": "Lifecycle",
  "method": "init",
  "error_sites": [
    {
      "line": 42,
      "expr": "self.clock.enable()?",
      "operation": "clock_management.enable",
      "fallible": true,
      "category": "ResourceUnavailable",
      "propagation": {
        "style": "question_mark_operator",
        "target": "Lifecycle::init return type",
        "effect": "propagates_to_caller"
      },
      "context": {
        "construct": "clock_management",
        "resource_id": "uart_clk",
        "phase": "init_preamble"
      }
    },
    {
      "line": 48,
      "expr": "self.regs.map()?",
      "operation": "register_access.map_region",
      "fallible": true,
      "category": null,  // map_region is compiler-generated, not authored
      "propagation": {
        "style": "question_mark_operator",
        "target": "Lifecycle::init return type",
        "effect": "propagates_to_caller"
      },
      "context": {
        "construct": "register_access",
        "resource_id": "pl011_regs",
        "phase": "init_preamble"
      },
      "note": "compiler_generated_not_authored"
    }
  ]
}
```

### Artifact Components

- **line, expr**: Precise location for diagnostics
- **operation**: Fully qualified construct + primitive (e.g., `clock_management.enable`)
- **fallible**: Boolean (can this fail?)
- **category**: Enum variant (if authored) or null (if generated)
- **propagation**: How this error flows to the caller
- **context**: Which construct, which resource, which lifecycle phase
- **note**: Extra metadata (authored vs. generated, etc.)

This structure allows downstream analysis and lowering passes to reason about error flow without re-parsing the source.

---

## 7. Design Principle: Error Extraction as a First-Class Compiler Concern

**Key principle**: Treating error flow as a vague afterthought weakens the compiler's ability to generate correct code.

Concrete outcomes of precise error extraction:

1. **Correct cleanup order** in generated C (LIFO releases on error path)
2. **Correct error codes** mapping authored semantics to OS-native values
3. **Verifiable cleanup paths** (compiler can assert "all resources cleaned up on all error paths")
4. **Diagnostic guidance** (warnings about unreachable error paths, missing error categories)

None of these are possible if the compiler only sees "this returns Result and maybe it fails somewhere".

---

## Conclusion

The compiler **must extract authored `RegseqError` as a semantic signal**, not as noise. Every error site should produce a clear artifact capturing:
- Location and expression
- Specific error category
- Fallibility and propagation method
- Construct and resource context
- Control-flow consequence

This precision enables clean lowering, verifiable resource cleanup, and correct error-path code generation.
