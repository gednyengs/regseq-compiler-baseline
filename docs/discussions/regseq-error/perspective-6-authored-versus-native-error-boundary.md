# Perspective 6: Authored vs Native Error Boundary

## Position Statement

There is a **hard semantic boundary** between:

1. **Authored errors** — the spec author explicitly wrote code that returns `RegseqError`
2. **Native errors** — the generated backend code calls an OS API that returns a native error code

These are not the same, and conflating them loses critical information.

**Key thesis**: Native OS errors should propagate directly in the generated code, not be round-tripped through `RegseqError` and back. This preserves backend semantics (like Linux probe deferral) that the Regseq DSL does not directly express, and it keeps the authored error signal clean.

---

## 1. Walking the Init Preamble: Which Steps Are Authored vs Generated

The init preamble is the clearest place to see the boundary in action.

### Reference Lifecycle Ordering (From `compiler-baseline-overview.md`)

| Step | Construct | Generated action |
|---|---|---|
| 1 | Driver Lifecycle | `create_device` + `associate_private_data` |
| 2 | Bus Resource Management | `get_resource_handle` |
| 3 | Register Access | `map_region` |
| 4 | Peripheral clock management | `acquire_clock` + enable |
| 5 | Hardware reset sequencing | `acquire_reset` + deassert |
| 6 | Pin controller and GPIO | `acquire_pinctrl` + apply default |
| 7 | Firmware property access | `read_property` |
| 8 | Kernel timer lifecycle | `init_timer` |
| 9 | DMA Management | `alloc_coherent` + `request_channel` |
| 10 | Device interrupt setup | `register_handler` |
| 11 | Driver Lifecycle | **`Lifecycle::init()` body** (authored) |
| 12 | Device interrupt setup | `enable` |

### Which Are Compiler-Generated OS API Calls?

Steps 1–10 and 12 are **compiler-generated**. The spec author does not write the code that calls the OS API.

For example, at Step 4 (`acquire_clock`), the compiler generates something like:

**Rust facade (DSL surface):**
```rust
impl Clock {
    fn enable(&self) -> Result<(), RegseqError> {
        // Author does NOT write this
        // The compiler generates the wrapper
        platform_clock_enable(self.clock_id)
            .map_err(|e| RegseqError::ResourceUnavailable)
    }
}
```

**Generated C (Linux lowering):**
```c
int pl011_acquire_clock(struct pl011_driver_data *drvdata) {
    drvdata->clk = clk_get(dev, "uart_clk");
    if (IS_ERR(drvdata->clk)) {
        // Native error from clk_get is a ptr encoded error code
        int err = PTR_ERR(drvdata->clk);
        // Backend decision: wrap in RegseqError or propagate directly?
        dev_err(dev, "Failed to acquire clock: %d\n", err);
        return err;  // Native error
    }
    return clk_enable(drvdata->clk);
}
```

The key question: **What error does `pl011_acquire_clock` return to `Lifecycle::init`?**

### The Two Models

#### Model A: Wrap All Backend Errors in RegseqError

The backend converts every native error to a `RegseqError` variant:

```c
int pl011_acquire_clock(struct device *dev, struct clock *clk) {
    struct clk *handle = clk_get(dev, "uart_clk");
    if (IS_ERR(handle)) {
        // Native errno: -ENOENT, -ENODEV, -EPROBE_DEFER, etc.
        // Backend wraps it
        return REGSEQ_ERROR_RESOURCE_UNAVAILABLE;  // Always this
    }
    // ...
}
```

**Consequence**: Information is lost. Linux probe deferral (`-EPROBE_DEFER`) becomes indistinguishable from "device truly not found" (`-ENODEV`).

#### Model B: Propagate Native Errors Directly

Generated code returns the native error directly:

```c
int pl011_acquire_clock(struct device *dev, struct clock *clk) {
    struct clk *handle = clk_get(dev, "uart_clk");
    if (IS_ERR(handle)) {
        // Return native Linux errno
        return PTR_ERR(handle);  // Could be -ENOENT, -EPROBE_DEFER, etc.
    }
    // ...
}
```

The caller in `Lifecycle::init` receives a native error:

```c
int err = pl011_acquire_clock(dev, &drvdata->clk);
if (err) {
    if (err == -EPROBE_DEFER) {
        // Handle probe deferral: ask framework to retry
        dev_dbg(dev, "Probe deferred, will retry\n");
        return -EPROBE_DEFER;  // Framework handles retry
    } else {
        // Other error: log and fail
        dev_err(dev, "Clock acquisition failed: %d\n", err);
        return err;
    }
}
```

**Consequence**: Backend-specific semantics are preserved and can be handled meaningfully.

---

## 2. Failure Modes in the Init Preamble: Native vs Authored

### A. Clock Acquisition (`step 4`)

**Compiler-generated OS call**: `clk_get` (Linux)

**Possible native errors**:
- `-ENOENT`: Clock not found in this domain
- `-ENODEV`: Clock subsystem error
- `-EPROBE_DEFER`: Dependency not yet available (retry later)
- `-EINVAL`: Invalid clock name or parameters

**Should this wrap in RegseqError?** NO.

**Reason**: The author did not write any code here. The compiler injected this call. If we wrap in `RegseqError`, we lose the distinction between "clock truly missing" and "clock will be available soon, please retry". The lowering backend (Linux) understands `EPROBE_DEFER` semantically; there is no `RegseqError::ProbeDeferred` because the DSL has no concept of it.

**Correct handling**:
- Compiler-generated code returns native error directly
- If author has a `Lifecycle::init` body and wants to handle clock failure, they write explicit error handling code
- If author doesn't handle it, the native error propagates up, and the framework (Linux probe handler) gets `EPROBE_DEFER` and retries as needed

### B. Pin Controller Application (`step 6`)

**Compiler-generated OS call**: `pinctrl_select_state` (Linux) or equivalent (Zephyr)

**Possible native errors**:
- `-ENODEV`: Pin controller not found
- `-EINVAL`: Invalid pin configuration state
- `-EACCES`: Permission issue (in some frameworks)

**Should this wrap in RegseqError?** NO, for the same reason.

The author did not configure the pins explicitly in `Lifecycle::init`. The compiler applied the default pin state as part of the init preamble. If it fails, it fails for OS-specific reasons that the author cannot meaningfully distinguish or handle without backend knowledge.

### C. Register MMIO Mapping (`step 3`)

**Compiler-generated OS call**: `devm_ioremap_resource` (Linux)

**Possible native errors**:
- `-EINVAL`: Invalid resource range
- `-ENOMEM`: Not enough virtual address space
- `-EBUSY`: Resource already in use

**Should this wrap in RegseqError?** NO.

These are platform-level failures that the author cannot fix in the DSL. They indicate misconfiguration at the OS or platform level, not driver logic errors.

### D. DMA Channel Request (`step 9`)

**Compiler-generated OS call**: `dma_request_chan` (Linux)

**Possible native errors**:
- `-ENODEV`: No DMA engine available
- `-EBUSY`: No channels available on the DMA controller
- `-EPROBE_DEFER`: DMA engine not yet probed

**Should this wrap in RegseqError?** NO.

Again, this is backend-specific resource contention. The author has no DSL-level recovery option for "DMA engine busy". The lowering backend should return the native error so the OS framework can retry if appropriate.

### E. Author-Written Code in `Lifecycle::init` Body (`step 11`)

**Authored code**:
```rust
impl Lifecycle for Pl011 {
    fn init(&mut self) -> Result<(), RegseqError> {
        // Author explicitly calls a fallible wrapper method
        self.uart.configure(&config)?;

        // Author manually checks a condition
        if self.expected_feature_flag.is_none() {
            return Err(RegseqError::ConfigurationMissing);
        }

        // Author delegates error handling
        self.setup_dma().map_err(|_| RegseqError::ResourceUnavailable)?;

        Ok(())
    }
}
```

**These errors MUST use RegseqError**.

- The author explicitly constructed them
- The author chose the semantic category
- The author's intent is to signal a failure that higher-level code (the framework or user) can act on

**Key distinction**: The author is **expressing logical failure**, not reporting OS API failures.

---

## 3. The Probe Deferral Problem: A Concrete Example

This is the sharpest illustration of why the boundary matters.

### Linux Probe Deferral Pattern

In Linux, when a device driver depends on another subsystem (clock, reset, DMA engine) that isn't ready yet, the probe should return `-EPROBE_DEFER`:

```c
static int pl011_probe(struct platform_device *pdev) {
    struct pl011 *dev = devm_kzalloc(&pdev->dev, ...);

    // Try to get the clock
    dev->clk = devm_clk_get(&pdev->dev, NULL);
    if (IS_ERR(dev->clk)) {
        int err = PTR_ERR(dev->clk);
        if (err == -EPROBE_DEFER)
            return -EPROBE_DEFER;  // CRITICAL: let framework retry
        dev_err(&pdev->dev, "failed to get clock: %d\n", err);
        return err;
    }

    // ... more initialization

    return 0;
}
```

If the compiler wraps the clock acquisition error in `RegseqError`, this pattern breaks:

```c
// WRONG MODEL: wrapping native errors in RegseqError
int err = pl011_acquire_clock(&pdev->dev, ...);
if (err) {
    // We've lost the -EPROBE_DEFER signal!
    // err is now REGSEQ_ERROR_RESOURCE_UNAVAILABLE for all failures
    return err;  // Framework doesn't recognize this code
}
```

The framework expects `-EPROBE_DEFER` (typically `-517` on Linux) to trigger retry logic. If we return an artificial `REGSEQ_ERROR_*` code instead, the framework doesn't recognize it and the probe fails permanently instead of deferring.

### Correct Approach

Generated code returns native errors directly:

```c
// Compiler-generated step 4: acquire_clock
int pl011_init_preamble_step4(struct pl011 *dev) {
    dev->clk = devm_clk_get(dev->dev, "uart_clk");
    if (IS_ERR(dev->clk))
        return PTR_ERR(dev->clk);  // Return native errno
    return clk_enable(dev->clk);
}

// Within Lifecycle::init (authored callback)
int pl011_lifecycle_init(struct pl011 *dev) {
    int err = pl011_init_preamble_step4(dev);
    if (err) {
        if (err == -EPROBE_DEFER) {
            dev_dbg(dev->dev, "Probe deferred\n");
            return -EPROBE_DEFER;  // Let framework retry
        }
        // Handle other errors if desired, or propagate
        return err;
    }

    // ... author's own init logic using RegseqError if needed ...

    return 0;
}
```

Or, if the author doesn't need to inspect the error specially:

```c
int err = pl011_init_preamble_step4(dev);
if (err)
    return err;  // Propagate native error directly to probe handler
```

The probe handler (framework-level) gets the correct error code and can retry.

### Zephyr Contrast: -ENOTSUP

Zephyr has a different paradigm. Some operations return `-ENOTSUP` to mean "this platform doesn't support this feature". For example:

```c
int err = pinctrl_apply(dev, "default");
if (err == -ENOTSUP) {
    // Pin control is optional on this platform, continue
    dev_warn(dev, "pin control not supported\n");
} else if (err) {
    // Real error
    return err;
}
```

The same principle applies: **if you wrap `-ENOTSUP` in `RegseqError::ResourceUnavailable`, you lose the information that this is an optional-feature signal, not a hard error.**

---

## 4. Should Native OS Errors Propagate Directly or Wrap in RegseqError?

**Answer: Propagate directly from compiler-generated code.**

### The Criterion

**For each generated operation in the init preamble, ask:**

> "Is there a meaningful `RegseqError` variant that fully captures the semantic intent of all possible native OS errors this operation can return?"

If NO (which is almost always the case), propagate native errors directly.

### Application

| Operation | Possible Native Errors | Meaningful RegseqError? | Propagate |
|---|---|---|---|
| `devm_clk_get` | `-ENOENT`, `-ENODEV`, `-EPROBE_DEFER` | NO (EPROBE_DEFER has no DSL equivalent) | Native directly |
| `devm_ioremap_resource` | `-EINVAL`, `-ENOMEM`, `-EBUSY` | NO (platform-level issues) | Native directly |
| `pinctrl_select_state` | `-ENODEV`, `-EINVAL`, `-ENOTSUP` | NO (ENOTSUP is a feature signal) | Native directly |
| `dma_request_chan` | `-ENODEV`, `-EBUSY`, `-EPROBE_DEFER` | NO (EPROBE_DEFER, resource contention) | Native directly |
| `devm_irq_get_irq` | `-EINVAL`, `-ENOMEM` | NO (platform-level) | Native directly |
| `request_irq` | `-EINVAL`, `-EBUSY` | NO (resource contention) | Native directly |

**All compiler-generated preamble operations propagate native errors directly.**

### Exception: Authored Wrapper Methods

When the author writes a wrapper method that calls an OS API:

```rust
impl Clock {
    fn enable(&self) -> Result<(), RegseqError> {
        // Author chooses to abstract this away
        platform_clock_enable(self.clock_id)
            .map_err(|_| RegseqError::ResourceUnavailable)
    }
}
```

The author has **decided to unify multiple native errors under one semantic category**. This is the author's choice, and it's valid: the author is saying "I don't want to expose platform details to my callers; all clock failures are 'resource unavailable' to them."

This is a localized wrapping decision, not a global pattern.

---

## 5. The Goto-Cleanup Chain and Error Paths

In Linux generated code, cleanup happens via goto chains.

### Error Path with Native Errors

```c
int pl011_init(struct device *dev, struct pl011 *drvdata) {
    int err = 0;

    // Step 3: map_region
    drvdata->regs_base = devm_ioremap_resource(dev, res);
    if (IS_ERR(drvdata->regs_base)) {
        err = PTR_ERR(drvdata->regs_base);
        dev_err(dev, "Failed to map MMIO region: %d\n", err);
        goto error_ioremap;
    }

    // Step 4: acquire_clock
    drvdata->clk = devm_clk_get(dev, "uart_clk");
    if (IS_ERR(drvdata->clk)) {
        err = PTR_ERR(drvdata->clk);
        if (err != -EPROBE_DEFER)
            dev_err(dev, "Failed to get clock: %d\n", err);
        goto error_clk_get;  // Unmap MMIO
    }

    err = clk_enable(drvdata->clk);
    if (err) {
        dev_err(dev, "Failed to enable clock: %d\n", err);
        goto error_clk_enable;  // Put clock, unmap MMIO
    }

    // Step 11: author's Lifecycle::init
    err = pl011_lifecycle_init(drvdata);
    if (err) {
        dev_err(dev, "Lifecycle init failed: %d\n", err);
        goto error_lifecycle;  // Disable clock, put clock, unmap MMIO
    }

    return 0;

error_lifecycle:
    clk_disable(drvdata->clk);
error_clk_enable:
    clk_put(drvdata->clk);
error_clk_get:
    // devm_ functions auto-cleanup, but explicit unmap needed in some cases
    // iounmap(drvdata->regs_base);
error_ioremap:
    return err;  // Propagate native error (or RegseqError from lifecycle)
}
```

**Key observations**:

1. **Native errors** (from `clk_get`, `clk_enable`, `ioremap`) propagate directly
2. **RegseqError** from `Lifecycle::init` also propagates (backend has converted it to native errno)
3. **Cleanup is ordered by dependency**: clock before MMIO (reverse acquisition order)
4. **Special handling** for `EPROBE_DEFER` is possible at the error-return site

This is clean and correct **only if we don't try to wrap native errors in RegseqError**. If we did, we'd have:

```c
// WRONG: wrapping native errors
int regseq_error = regseq_acquire_clock(...);
if (regseq_error) {
    // We've lost the EPROBE_DEFER signal!
    return regseq_error;  // But this isn't a real errno anymore
}
```

The probe handler wouldn't understand `REGSEQ_ERROR_*` codes.

---

## 6. Drawing the Explicit Boundary

### Authored Error Signals

These **must** use `RegseqError`:

- Explicit `RegseqError::*` construction by the author
- Errors returned by author-written helper functions (functions in the impl block)
- Errors propagated from wrapper methods that the author called

Example:
```rust
fn init(&mut self) -> Result<(), RegseqError> {
    // All of these are AUTHORED
    self.clock.enable()?;               // Author called the wrapper
    if config.is_none() {
        return Err(RegseqError::ConfigurationMissing);  // Authored
    }
    self.custom_helper()?;              // Author-written function
    Ok(())
}

fn custom_helper(&mut self) -> Result<(), RegseqError> {
    // Author-written code
    Err(RegseqError::InvalidParameter)
}
```

### Native Error Signals

These **should NOT wrap in RegseqError**:

- Errors from `devm_clk_get`, `clk_enable` → propagate `-ENOENT`, `-EPROBE_DEFER`, etc.
- Errors from `devm_ioremap_resource` → propagate `-ENOMEM`, `-EINVAL`
- Errors from `pinctrl_select_state` → propagate `-ENODEV`, `-ENOTSUP`
- Errors from `dma_request_chan` → propagate `-EBUSY`, `-EPROBE_DEFER`
- Errors from `request_irq` → propagate `-EBUSY`, `-EINVAL`

These are **compiler-generated OS API calls** and their native errors carry semantic information that the backend needs to preserve.

### The Boundary Line

**Authored code** = code the spec author explicitly wrote in the DSL (trait impl methods, helper functions, attributed handler bodies).

**Generated code** = code the compiler injected as part of the init/deinit/suspend/resume lifecycle or wrapper glue.

**Rule**:
- Authored code can return `RegseqError` and should
- Generated code returns native errors from OS APIs directly
- If generated code needs to return a non-zero value (error), it returns the native code
- Errors from authored code (RegseqError) are converted to native errors by the lowering backend for propagation to the OS framework

---

## 7. Zephyr vs Linux: Different Native Semantics

### Linux

Probe deferral is a first-class concept:
- Returns `-EPROBE_DEFER` (`-517`)
- Framework recognizes and retries
- Must propagate through the error chain to reach probe handler

### Zephyr

Different feature signaling:
- `-ENOTSUP` means "this feature isn't supported on this platform"
- Not always a hard error; driver may continue with degraded operation
- Must propagate to allow optional-feature detection

### Both

**Losing native error codes by wrapping in RegseqError breaks these mechanisms.**

---

## 8. Concrete Lowering Examples

### Example 1: Clock Acquisition on Linux

**Rust DSL:**
```rust
impl Lifecycle for Pl011 {
    fn init(&mut self) -> Result<(), RegseqError> {
        self.clock.enable()?;  // Author uses wrapper
        // ...
        Ok(())
    }
}
```

**Generated C (compiler produces this):**
```c
// Compiler-generated preamble step 4
int pl011_acquire_and_enable_clock(struct pl011 *dev) {
    dev->clk = devm_clk_get(dev->dev, "uart_clk");
    if (IS_ERR(dev->clk)) {
        int err = PTR_ERR(dev->clk);
        // DON'T wrap: return native error
        return err;  // Could be -ENOENT, -EPROBE_DEFER, -ENOMEM
    }
    err = clk_enable(dev->clk);
    return err;  // Could be 0 or -EINVAL, etc.
}

// Author's Lifecycle::init (authored)
int pl011_lifecycle_init(struct pl011 *dev) {
    // Author's code runs after preamble
    // If author called self.clock.enable(), that was already done in preamble
    // Author's code here is additional logic
    return 0;  // Or return -ESOMETHING if author's logic fails
}

// Probe handler (framework level)
static int pl011_probe(struct platform_device *pdev) {
    struct pl011 *dev = devm_kzalloc(...);

    int err = pl011_acquire_and_enable_clock(dev);
    if (err) {
        if (err == -EPROBE_DEFER) {
            // Framework handles this: schedule retry
            return -EPROBE_DEFER;
        }
        // Other error
        dev_err(&pdev->dev, "Failed to acquire clock: %d\n", err);
        return err;
    }

    err = pl011_lifecycle_init(dev);
    if (err) {
        dev_err(&pdev->dev, "Lifecycle init failed: %d\n", err);
        return err;  // Propagate to framework
    }

    return 0;
}
```

**Critical**: The native error code flows directly from OS API → probe handler. No RegseqError wrapping.

### Example 2: Author-Written Helper with RegseqError

**Rust DSL:**
```rust
impl Pl011 {
    fn configure(&mut self) -> Result<(), RegseqError> {
        if self.baud_rate == 0 {
            return Err(RegseqError::InvalidParameter);
        }
        Ok(())
    }
}

impl Lifecycle for Pl011 {
    fn init(&mut self) -> Result<(), RegseqError> {
        self.configure()?;
        Ok(())
    }
}
```

**Generated C:**
```c
// Authored helper
int pl011_configure(struct pl011 *dev) {
    if (dev->baud_rate == 0) {
        // Author authored this error check
        // Backend converts RegseqError::InvalidParameter to native code
        return -EINVAL;  // Linux errno
    }
    return 0;
}

// Authored Lifecycle::init
int pl011_lifecycle_init(struct pl011 *dev) {
    int err = pl011_configure(dev);
    if (err)
        return err;  // Propagate (already native errno from backend conversion)
    return 0;
}
```

**Key**: The backend (during lowering to C) converted `RegseqError::InvalidParameter` → `-EINVAL` in the implementation of `configure`. The authored code uses `RegseqError`, but the generated C uses native errors.

---

## Conclusion

### The Boundary is Clear

| Signal | Source | Type | Propagation |
|---|---|---|---|
| Clock unavailable | `devm_clk_get` in init preamble | Native error code | Direct to probe handler |
| Probe deferred | `devm_clk_get` in init preamble | Native error code | Direct to probe handler (framework handles retry) |
| Invalid configuration | Author checked a condition | `RegseqError` in DSL | Backend converts to native, propagates |
| Resource exhausted (DMA) | `dma_request_chan` in init preamble | Native error code | Direct to probe handler |
| Pin configuration unsupported | `pinctrl_select_state` in init preamble | Native error code | Direct to probe handler (author can detect and continue) |

### Design Principle

**Generated preamble code returns native errors. Authored callback code returns RegseqError. The boundary is the authored/generated code split.**

This preserves:
- Backend-specific semantics (probe deferral, optional features)
- Clear extraction signals (authored errors only in authored code)
- Correct error handling (framework gets native codes it understands)
- Minimal wrapping overhead (no redundant error conversions)

Violating this boundary (by wrapping native errors in RegseqError) breaks probe deferral, loses platform-specific signal semantics, and makes lowering fragile.

