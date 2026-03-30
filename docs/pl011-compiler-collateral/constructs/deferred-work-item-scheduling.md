# Deferred Work Item Scheduling

## Purpose

`Deferred work item scheduling` defines the typed work-item resource surface,
attribute-bound work-handler methods, work-submission wrapper methods, and the
compiler-managed work-item lifecycle.

This is the explicit driver-facing deferred-execution construct. It is distinct
from Timer `auto_routing`, which remains compiler-internal to the timer
construct.

## Accepted Primitive Inventory

### Layer 1

| Primitive | Kind | Stable surface |
|---|---|---|
| `deferred_work.work` | resource | typed field `Work` / `Option<Work>` |
| `deferred_work.handler_binding` | handler_binding | `#[rseq::work_handler]` on a driver method |
| `deferred_work.submit` | operation | `Work::submit(&self)` |
| `deferred_work.submit_unique` | operation | `Work::submit_unique(&self)` |

### Layer 2

This construct has no direct Layer 2 declarations.

### Compiler-managed

| Primitive | Role |
|---|---|
| `deferred_work.init_work` | initialize/register work item |
| `deferred_work.cancel_work` | cancel/flush and tear down work item |

## Accepted Surface Baseline

### Wrapper baseline

Stable typed resource:

```rust
struct E1000 {
    link_work: Work,
}
```

Stable wrapper surface:

```rust
impl Work {
    fn submit(&self);
    fn submit_unique(&self);
}
```

Accepted decisions:

- both submission methods are infallible
- `submit_unique()` is the coalescing variant
- work submission is driver-authored and may happen from ISR, timer, or process
  context

### Bound-handler baseline

Stable binding forms:

```rust
impl Driver {
    #[rseq::work_handler]
    fn handle_work(&mut self) { /* ... */ }
}
```

```rust
impl Driver {
    #[rseq::work_handler(link_work)]
    fn handle_link(&mut self) { /* ... */ }
}
```

Accepted bound-handler rules:

- callback entry context: `H` (explicit handler-managed entry)
- work handlers return `()`
- wrappers recover context but do not auto-acquire locks
- bare auto-bind is permitted only when exactly one `Work` field exists

## Extraction Baseline

The frontend must recognize:

- `Work` / `Option<Work>` typed fields
- `#[rseq::work_handler]` and `#[rseq::work_handler(field_name)]`
- wrapper-method calls `submit()` and `submit_unique()`

Primary specification signals:

- typed work field declarations
- method attributes for handler binding
- wrapper-method call sites for submission

Required extraction validations:

- bare `#[rseq::work_handler]` only when the work resource is unique
- explicit field binding is required when multiple work items exist
- each work field has exactly one bound handler
- work handlers remain void-returning bound methods

## Lowering Baseline

Lowering ownership:

- handler binding: backend realizes OS-specific work-wrapper glue
- submission methods: backend lowers to target work-queue submission APIs
- init/cancel lifecycle: compiler-generated

### Generated lifecycle phase table

| Primitive | Init preamble | Deinit postamble | Suspend postamble | Resume preamble |
|---|---|---|---|---|
| `init_work` | yes | no | no | no |
| `cancel_work` | no | yes | no | no |

Accepted lowering notes:

- work handlers always run in deferred/thread context
- explicit `Work` remains distinct from timer `auto_routing`
- `submit_unique()` coalescing may require compiler help on targets without
  inherent queue coalescing

## Cross-Construct Dependencies

Depends on:

- `Synchronization` for handler-managed lock discipline
- `Device interrupt setup and handling` and `Kernel timer lifecycle
  management` as common submission sources

Feeds:

- `Network Interface Management` and other device-class work that must leave
  ISR/timer context

## Constraints and Caveats

- Keep work handlers on the bound-handler surface, not on the trait callback
  surface.
- Preserve the no-auto-lock handler model.
- Keep explicit `Work` distinct from timer `auto_routing`.
- Keep lifecycle ownership on `init_work` / `cancel_work`, not on the typed
  `Work` resource record.

## Representative Examples

```rust
struct E1000 {
    link_work: Work,
}

impl E1000 {
    #[rseq::work_handler]
    fn handle_link_work(&mut self) {
        // deferred link/update handling
    }
}
```

## Implementation Checklist

- Recognize `Work` fields and all accepted `#[rseq::work_handler]` forms.
- Lower `submit()` and `submit_unique()` with the accepted coalescing model.
- Generate `init_work` in init and `cancel_work` in deinit.
- Keep work handlers in class `H` with explicit driver-managed lock scope.
- Keep timer `auto_routing` and explicit `Work` lowering as distinct paths.
