# Kernel Timer Lifecycle Management

## Purpose

`Kernel timer lifecycle management` defines the software-timer resource
 surface, attribute-bound timer-handler methods, wrapper-based timer control,
 and the compiler-generated timer lifecycle/runtime glue.

## Accepted Primitive Inventory

### Layer 1

| Primitive | Kind | Stable surface |
|---|---|---|
| `timer_management.timer_id` | property | `#[rseq::timer(id = "...")]` |
| `timer_management.timer_mode` | property | `#[rseq::timer(mode = "oneshot" | "periodic")]` |
| `timer_management.interval` | property | `interval_ms = ...` or `interval_from = "field"` |
| `timer_management.tolerance` | property | `tolerance_ms = ...` |
| `timer_management.resolution` | property | `resolution = "standard" | "high"` |
| `timer_management.timer` | resource | typed field `Timer` / `Option<Timer>` |
| `timer_management.handler_binding` | handler_binding | `#[rseq::timer_handler]` on a driver method |
| `timer_management.start` | operation | `Timer::start(&self)` |
| `timer_management.start_with` | operation | `Timer::start_with(&self, duration: Duration)` |
| `timer_management.stop` | operation | `Timer::stop(&self)` |

### Layer 2

This construct has no direct Layer 2 declarations. It may consume property-fed
 values indirectly through `interval_from`.

### Compiler-managed

| Primitive | Role |
|---|---|
| `timer_management.init_timer` | create/destroy timer object and registration glue |
| `timer_management.auto_routing` | compile-time routing of timer handlers to direct or deferred execution |

## Accepted Surface Baseline

### Wrapper baseline

Stable timer surface:

```rust
impl Timer {
    fn start(&self);
    fn start_with(&self, duration: Duration);
    fn stop(&self);
}
```

Accepted decisions:

- all three methods are infallible
- all three use `&self`
- periodic rearm is compiler-generated and transparent
- `no_wake` is not part of the stable Layer 1 baseline

### Bound-handler baseline

Stable timer handler form:

```rust
impl Driver {
    #[rseq::timer_handler(poll_timer)]
    fn poll_expiry(&mut self) { /* ... */ }
}
```

Accepted bound-handler rules:

- callback entry context: `H` (explicit handler-managed entry)
- timer handlers return `()`
- wrappers recover context but do not auto-acquire locks
- single-resource auto-bind is permitted for bare `#[rseq::timer_handler]`
  when timer type is unique

## Extraction Baseline

The frontend must recognize:

- timer field attributes for id/mode/interval/tolerance/resolution
- `Timer` / `Option<Timer>` typed fields
- `#[rseq::timer_handler]` binding forms
- wrapper-method calls `start()`, `start_with(Duration)`, `stop()`
- `interval_from` references to populated property fields

Primary specification signals:

- field attributes for timer declaration
- typed timer field declaration
- method attributes for handler binding
- wrapper-method call sites for control operations

Required extraction validations:

- `interval_ms` and `interval_from` are mutually exclusive
- periodic vs oneshot mode must align with rearm expectations
- auto-binding only when the timer resource is unique
- handlers remain void-returning bound methods

## Lowering Baseline

Lowering ownership:

- timer object creation/destruction and handler registration: compiler-generated
- direct vs auto-routed callback path: compiler-generated from body analysis
- wrapper methods: backend lowers timer-control operations

### Generated lifecycle phase table

| Primitive | Init preamble | Deinit postamble | Suspend postamble | Resume preamble |
|---|---|---|---|---|
| `init_timer` | yes | yes | no | no |
| `auto_routing` | compile-time analysis | compile-time analysis | no | no |

Accepted runtime/lowering notes:

- direct-path timer handlers run in non-sleepable timer context
- auto-routed handlers run through a two-stage callback shape into sleepable
  workqueue/thread context
- `auto_routing` analysis consumes three signal categories from the handler
  body:
  - intrinsics that require sleepable context
  - typed-resource wrapper calls that require sleepable context
  - device-class callback invocations
- the current accepted trigger set is:
  - `regseq::delay::sleep(...)`
  - `Mutex::lock()`
  - `Completion::wait(...)`
  - framework/device-class callback invocations that assume process/thread
    context
- the current accepted non-triggering set is:
  - register reads/writes
  - `IrqSpinLock::lock()`
  - `SpinLock::lock()`
  - `regseq::delay::busy_wait(...)`
  - `Completion::signal()`
  - `Completion::reset()`
  - `regseq::log::*` macros
  - `Timer::start()` / `start_with()` / `stop()`
- decision rule: if any trigger is found in the handler body, enable
  `auto_routing`; otherwise use the direct path
- periodic rearm is compiler-generated after handler return
- coalescing of timer firings during deferred-work latency is a compiler
  requirement
- `stop()` from within a handler suppresses rearm

Accepted PM rule:

- timer PM behavior is class `V`
- stop/restart during PM is driver-authored and compiler-validated
- timers are not part of the generated PM sequence

## Cross-Construct Dependencies

Depends on:

- `Synchronization` for handler-managed lock discipline and wait legality
- `Timed CPU delay and sleep` / `Duration` utility
- `Device power state management` for PM interaction rules
- `Firmware property access` when `interval_from` references populated fields

Feeds:

- `Deferred work item scheduling` assumptions for auto-routed handlers
- device-class constructs such as Serial where timers support background work

## Constraints and Caveats

- Keep timer handlers on the bound-handler surface, not on the trait callback
  surface.
- Preserve the no-auto-lock handler model.
- Preserve the accepted two-path auto-routing model.
- Preserve the accepted exclusion of `no_wake` from the stable Layer 1 surface.
- Timer stop/restart during PM remains driver-authored and compiler-validated.

## Representative Examples

```rust
struct Pl011 {
    #[rseq::timer(id = "dma_rx_poll", mode = "periodic", interval_from = "poll_rate")]
    poll_timer: Timer,
}

impl Pl011 {
    #[rseq::timer_handler(poll_timer)]
    fn dma_rx_poll_expiry(&mut self) {
        // handler body
    }
}
```

## Implementation Checklist

- Recognize timer declarations and handler-binding forms.
- Lower `start`, `start_with`, and `stop` with the accepted `&self` / infallible
  surface.
- Implement `auto_routing` analysis and the two-stage callback path.
- implement the accepted trigger/non-trigger signal set directly in timer
  lowering.
- Implement compiler-generated periodic rearm semantics.
- Enforce interval exclusivity and binding-form validation.
- Keep PM stop/restart in driver-authored code with compiler validation only.
