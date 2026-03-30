# Atomic Memory Access And Ordering

## Purpose

`Atomic memory access and ordering` defines the typed `Atomic<T>` shared-state
surface and its explicit ordering-aware wrapper methods.

This is a typed-resource wrapper construct. It has no trait, bound-handler
surface, Layer 2 binding data, or separate compiler-managed
lifecycle primitive.

## Accepted Primitive Inventory

### Layer 1

| Primitive | Kind | Stable surface |
|---|---|---|
| `atomic_operations.atomic_type` | property | typed field `Atomic<T>` |
| `atomic_operations.initial_value` | property | `#[rseq::atomic(initial = ...)]` field attribute |
| `atomic_operations.load` | operation | `Atomic<T>::load(&self, order)` |
| `atomic_operations.store` | operation | `Atomic<T>::store(&self, value, order)` |
| `atomic_operations.fetch_modify` | operation | std-style `fetch_*` / `swap` wrapper family |
| `atomic_operations.compare_exchange` | operation | `Atomic<T>::compare_exchange(...) -> Result<T, T>` |
| `atomic_operations.bit_test_set` | operation | bit-test wrapper methods on unsigned integer atomics |

### Layer 2

This construct has no direct Layer 2 declarations. Atomic state is driver-local
shared state, not a deployment/binding resource.

### Compiler-managed

This construct has no separate compiler-managed primitive. Initial value setup
is emitted as part of field initialization / setup lowering, not as a distinct
lifecycle primitive.

## Accepted Surface Baseline

### Typed-resource baseline

Stable declaration form:

```rust
struct MyDriver {
    #[rseq::atomic(initial = 0)]
    tx_active: Atomic<u32>,
}
```

Accepted decisions:

- `Atomic<T>` is the typed shared-state carrier
- the current stable baseline uses `#[rseq::atomic(initial = ...)]` for initial
  value declaration
- Atomic remains scoped to ordinary shared-state atomic access, not MMIO/device
  ordering

### Ordering baseline

Stable ordering surface follows Rust-style ordering:

```rust
enum Ordering {
    Relaxed,
    Acquire,
    Release,
    AcqRel,
    SeqCst,
}
```

All atomic operations take explicit ordering parameters. There is no implicit
default ordering in the stable baseline.

### Wrapper baseline

Stable wrapper-family conventions:

```rust
impl<T: AtomicInt> Atomic<T> {
    fn load(&self, order: Ordering) -> T;
    fn store(&self, value: T, order: Ordering);
    fn fetch_add(&self, value: T, order: Ordering) -> T;
    fn fetch_sub(&self, value: T, order: Ordering) -> T;
    fn fetch_or(&self, value: T, order: Ordering) -> T;
    fn fetch_and(&self, value: T, order: Ordering) -> T;
    fn fetch_xor(&self, value: T, order: Ordering) -> T;
    fn swap(&self, value: T, order: Ordering) -> T;
    fn compare_exchange(
        &self,
        current: T,
        new: T,
        success: Ordering,
        failure: Ordering,
    ) -> Result<T, T>;
}

impl Atomic<u32> {
    fn bit_test_set(&self, bit: u32, order: Ordering) -> bool;
    fn bit_test_clear(&self, bit: u32, order: Ordering) -> bool;
}
```

Accepted wrapper conventions:

- all methods use `&self`
- std-style naming is the stable baseline
- `fetch_modify` in the primitive inventory maps to the concrete `fetch_*` /
  `swap` method family
- `compare_exchange` uses dual ordering and a domain-specific `Result<T, T>`
  outcome

### Excluded baseline

- MMIO barrier/fence elements are not owned by Atomic
- `register_access.io_fence` remains excluded from the stable base surface
- lock-based synchronization remains part of `Synchronization`, not Atomic

## Extraction Baseline

The frontend must recognize:

- `Atomic<T>` typed fields and their element type
- `#[rseq::atomic(initial = ...)]` field attributes
- wrapper-method calls on `Atomic<T>`
- explicit ordering arguments at call sites

Primary specification signals:

- field type for `atomic_type`
- field attribute arguments for `initial_value`
- wrapper-method call sites for atomic operations

Required extraction validations:

- `T` must be a supported atomic element type
- every atomic operation must carry explicit ordering
- `bit_test_*` methods are only valid on supported unsigned integer atomics
- MMIO/barrier evidence must not be reclassified back into Atomic

## Lowering Baseline

Lowering ownership:

- wrapper methods: backend lowers to target atomic APIs
- initial-value setup: compiler emits field initialization/setup code, but
  there is no separate Atomic lifecycle primitive

### Generated lifecycle phase table

This construct has no generated lifecycle primitive table. Atomic state is
driver-local shared state with no acquire/release lifecycle.

Accepted lowering notes:

- Atomic is the only typed-resource wrapper construct with no distinct
  compiler-managed lifecycle primitive
- explicit ordering stays part of the user-facing stable surface
- compare-exchange failure is semantic contention, not `RegseqError`

## Cross-Construct Dependencies

Related to:

- `Synchronization`, which covers lock/completion coordination rather than
  atomic shared-state access
- `Register Access`, which owns MMIO/device-ordering concerns and the deferred
  barrier/backpressure questions

Feeds:

- any driver logic that needs lock-free shared-state coordination across ISR,
  timer, work, or process contexts

## Constraints and Caveats

- Keep Atomic scoped to typed `Atomic<T>` shared-state operations.
- Preserve std-style naming with explicit `Ordering`.
- Keep `compare_exchange` as `Result<T, T>`, not `RegseqError`.
- Preserve the boundary that moved MMIO barrier/fence evidence into Register
  Access.
- Do not introduce a separate Atomic lifecycle primitive.

## Representative Examples

```rust
struct Pl011 {
    #[rseq::atomic(initial = 0)]
    tx_active: Atomic<u32>,
}

fn mark_tx_active(&self) {
    self.tx_active.store(1, Ordering::Release);
}
```

## Implementation Checklist

- Recognize `Atomic<T>` typed fields and supported element types.
- Recognize `#[rseq::atomic(initial = ...)]` and emit initial-value setup.
- Lower the std-style wrapper method family with explicit `Ordering`.
- Lower `compare_exchange` with separate success/failure ordering semantics.
- Keep bit-test methods restricted to supported unsigned integer atomic types.
- Preserve the accepted boundary between Atomic ordering and Register Access
  MMIO ordering.
