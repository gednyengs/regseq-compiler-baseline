# Timed CPU Delay And Sleep

## Purpose

`Timed CPU delay and sleep` defines the stable intrinsic surface for short
 busy-wait delays and scheduler-yielding sleeps, plus the context rules that
 govern where each is legal.

This is an intrinsic-only construct. It has no declarative properties, typed
 resources, traits, handlers, or compiler-managed lifecycle primitives.

## Accepted Primitive Inventory

### Layer 1

| Primitive | Kind | Stable surface |
|---|---|---|
| `timed_delay.busy_wait` | operation | `regseq::delay::busy_wait(Duration)` |
| `timed_delay.sleep` | operation | `regseq::delay::sleep(Duration)` |

### Layer 2

This construct has no Layer 2 declarations.

### Compiler-managed

This construct has no compiler-managed primitives of its own.

## Accepted Surface Baseline

Stable intrinsic surface:

```rust
regseq::delay::busy_wait(Duration::from_us(10));
regseq::delay::sleep(Duration::from_ms(100));
```

Accepted shared utility:

- `regseq::Duration` is the shared compile-time utility type
- stable constructors are:
  - `Duration::from_us(u64)`
  - `Duration::from_ms(u64)`
  - `Duration::from_s(u64)`

## Extraction Baseline

The frontend must recognize:

- `regseq::delay::busy_wait(Duration)`
- `regseq::delay::sleep(Duration)`
- `Duration::from_us/from_ms/from_s` constructor calls

Primary specification signals:

- intrinsic call sites
- `Duration` constructor calls

Required extraction validations:

- `sleep` only in sleepable contexts
- `busy_wait` always legal, but warn when long delays appear in clearly
  sleepable contexts
- keep Linux `cpu_relax()` poll-loop hints out of this construct; they belong
  to `Register Access` poll lowering

## Lowering Baseline

Lowering ownership:

- backend lowers both intrinsics to target delay/sleep APIs
- `Duration` is compile-time only; the backend converts to target units

### Generated lifecycle phase table

This construct has no generated lifecycle phases.

Accepted cross-construct rule:

- `busy_wait` and `sleep` legality is governed by the accepted callback-context
  model from `P1-03`

## Cross-Construct Dependencies

Shared dependency:

- `regseq::Duration` is also consumed by:
  - `Kernel timer lifecycle management`
  - `Synchronization` completion waits
  - `Register Access` poll delays/timeouts

Related dependency:

- `Register Access` poll lowering may internally use delay behavior, but
  poll-loop `cpu_relax()`/yield hints are not authored `timed_delay` calls

## Constraints and Caveats

- Preserve the hard boundary: `busy_wait` is valid in any context; `sleep` is
  only valid in sleepable contexts.
- Preserve the accepted reclassification of PL011 `cpu_relax()` polling hints
  as Register Access lowering details.
- Keep `Duration` as a shared compile-time utility rather than a runtime
  struct.

## Representative Examples

```rust
regseq::delay::busy_wait(Duration::from_us(100));
regseq::delay::sleep(Duration::from_ms(10));
```

## Implementation Checklist

- Recognize the two delay intrinsics and the three stable `Duration`
  constructors.
- Lower `busy_wait` and `sleep` to target APIs with correct unit conversion.
- Enforce the accepted sleep-context checks.
- Keep poll-loop processor hints out of the user-facing timed-delay surface.
