# Synchronization

## Purpose

`Synchronization` defines the typed lock/completion surface, the compiler-
 managed lifecycle for synchronization primitives, and the static-analysis
 contracts that govern lock legality and callback-context discipline.

This construct has no trait or bound-handler surface. It is a typed-wrapper
 plus compiler-analysis construct.

## Accepted Primitive Inventory

### Layer 1

| Primitive | Kind | Stable surface |
|---|---|---|
| `sync.lock_type` | property | typed field (`Mutex<T>`, `SpinLock<T>`, `IrqSpinLock<T>`, `Completion`) |
| `sync.protected_data_type` | property | generic parameter on lock-bearing wrappers |
| `sync.raii_guard` | property | `Guard<'a, T>` scope-bounded guard surface |
| `sync.lock` | operation | wrapper method `.lock()` |
| `sync.completion_wait` | operation | wrapper method `.wait(timeout)` |
| `sync.completion_signal` | operation | wrapper method `.signal()` |
| `sync.completion_reset` | operation | wrapper method `.reset()` |

### Layer 2

This construct has no direct Layer 2 declarations.

### Compiler-managed

| Primitive | Role |
|---|---|
| `sync.init_primitives` | initialize lock/completion backing state where target requires it |
| `sync.destroy_primitives` | destroy backing state where target requires it |
| `sync.subsystem_locking` | trait-callback entry locking analysis/emission for subsystem-serialized callbacks |

## Accepted Surface Baseline

### Lock baseline

Stable surface:

```rust
impl<T> Mutex<T> {
    fn lock(&self) -> Guard<'_, T>;
}

impl<T> SpinLock<T> {
    fn lock(&self) -> Guard<'_, T>;
}

impl<T> IrqSpinLock<T> {
    fn lock(&self) -> Guard<'_, T>;
}
```

Accepted decisions:

- `Guard<'a, T>` is the single stable guard type
- lock acquisition uses `&self` (interior mutability)
- `try_lock` is excluded from the stable base surface

### Completion baseline

Stable surface:

```rust
impl Completion {
    fn wait(&self, timeout: Duration) -> Result<(), RegseqError>;
    fn signal(&self);
    fn reset(&self);
}
```

### Callback-context relationship

This construct does not itself define callbacks, but it constrains them:

- trait-callback entry contexts:
  - `F` framework-managed entry
  - `S` subsystem-serialized entry
- bound-handler entry context:
  - `H` explicit handler-managed entry

Accepted P1-03 rule:

- ISR, Timer, and Work bound-handler wrappers do not auto-acquire locks
- handler bodies manage lock scopes explicitly using Synchronization primitives
- `subsystem_locking` applies to trait callback entry, not to bound handlers

## Extraction Baseline

The frontend must recognize:

- synchronization wrapper field types and generic parameters
- `.lock()`, `.wait(timeout)`, `.signal()`, `.reset()` wrapper calls
- guard-scope structure relevant to static checks
- callback surfaces that interact with synchronization legality rules

Primary specification signals:

- typed synchronization fields
- generic protected-data payloads
- wrapper-method call sites
- callback/handler surfaces consumed by static-analysis passes

Required extraction validations:

- only `IrqSpinLock` is ISR-safe
- `Completion::wait` is thread-context only
- framework calls must not occur inside forbidden guard scopes
- guard/data references must not escape locked scope

## Lowering Baseline

Lowering ownership:

- wrapper operations: backend lowers to target lock/completion APIs
- primitive initialization/destruction: compiler-generated where needed
- subsystem locking: compiler-managed analysis/emission for trait callback
  entry

### Generated lifecycle phase table

| Primitive | Init preamble | Deinit postamble | Suspend postamble | Resume preamble |
|---|---|---|---|---|
| `init_primitives` | yes | no | no | no |
| `destroy_primitives` | no | conditional | no | no |
| `subsystem_locking` | no | no | no | no |

Accepted notes:

- `destroy_primitives` remains target-conditional
- `subsystem_locking` is a compiler-managed analysis/lowering concern, not a
  lifecycle-step primitive

## Cross-Construct Dependencies

Constrains:

- `Device interrupt setup and handling`
- `Kernel timer lifecycle management`
- `Deferred work item scheduling`
- `Serial/UART Management`
- other trait-callback-bearing constructs through callback-context policy

Related dependencies:

- `Timed CPU delay and sleep` and `regseq::Duration` for completion waits
- `Register Access` for guard-scope and ISR legality interactions

## Constraints and Caveats

- Keep Synchronization non-trait-based.
- Preserve the single generic `Guard<'a, T>` baseline.
- Keep `try_lock` out of the stable base surface.
- Bound-handler wrappers must not auto-acquire locks.
- `subsystem_locking` belongs to trait callback entry and must not be widened
  into a generic bound-handler wrapper-locking mechanism.

## Representative Examples

```rust
struct CallbackState {
    // interrupt-path mutable state
}

struct Pl011 {
    cb_lock: IrqSpinLock<CallbackState>,
    xfer_done: Completion,
}

fn wait_xfer(&self, timeout: Duration) -> Result<(), RegseqError> {
    self.xfer_done.wait(timeout)
}
```

## Implementation Checklist

- Recognize synchronization wrapper types and generic payloads.
- Lower `.lock()` to the correct backend primitive by lock kind.
- Lower `Completion` operations with the accepted signatures.
- Implement the accepted static-analysis constraints around ISR legality,
  wait-context legality, and guard scope.
- Keep `subsystem_locking` separate from bound-handler lock realization.
- Treat `destroy_primitives` as conditional in backend implementation.
