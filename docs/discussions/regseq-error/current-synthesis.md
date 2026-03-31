# RegseqError Current Synthesis

## Purpose

This note consolidates the current state of the `RegseqError` discussion after
the initial design debate and the first round of external review.

It is not a final resolution. Its job is to make the current direction explicit
before we ask for another round of critique.

## Current Position

The current direction is:

1. `RegseqError` is the compiler-facing common error space.
2. `RegseqError` is a semantic DSL abstraction, not a Rust runtime error
   framework.
3. Semantic `RegseqError` leaves authored DSL only at outermost authored
   fallible boundaries; in the current design, those are fallible Regseq trait
   callback exits.
4. Helper functions are semantic-flow zones, not commit points.
5. Backend-native operational failure in authored DSL should enter through
   fallible intrinsics, not through trait callbacks.
6. Flattening every fallible operation directly to `RegseqError` is too coarse.
7. Intrinsic- or construct-local error types should be allowed to convert into
   `RegseqError`.

## What RegseqError Is

`RegseqError` is the shared compiler-facing error enum for authored semantic
 failure in the Regseq DSL.

It exists so that:

- fallible trait callback exits have one common lowering contract
- the compiler has one common semantic error space to analyze
- authored helper code can converge on a shared error carrier when needed

`RegseqError` is not:

- a direct wrapper around Linux errno, Zephyr errno, NTSTATUS, or any other
  target-native status family
- a `std::error::Error`-style runtime error object
- the only error type authors are ever allowed to name locally

## What RegseqError Should Not Depend On

The current direction remains:

- do not require `std::error::Error`
- do not make `Display` a defining requirement
- do not fuse logging into the error model

This keeps `RegseqError` tied to compiler semantics rather than host-runtime
 conventions.

## Outermost Authored Fallible Boundaries

The most important narrowing from the discussion is this:

- inside authored DSL code, `RegseqError` may remain abstract for a long time
- it does not need backend-specific materialization at every `Err(...)` or `?`
- semantic `RegseqError` only needs backend-specific materialization when it
  escapes the authored DSL world
- in the current design, the places where that escape happens are the exits of
  fallible Regseq trait callback methods

Examples:

- `Lifecycle::init`
- `Lifecycle::deinit`
- `PowerManagement::suspend`
- `PowerManagement::resume`
- `Serial::port_open`
- `Serial::configure`

This means:

- helper functions returning `RegseqResult<_>` are not outermost authored
  fallible boundaries on their own
- local matching and reclassification are not commit points
- callback-exit lowering is where semantic `RegseqError` becomes target-native

### Why trait callback exits have this role

The reason is not merely that traits are special. The reason is boundary
 ownership.

Inside helper functions or other authored DSL functions, a semantic error still
 has an authored caller above it. That means it can continue to flow abstractly
 as a semantic Regseq value.

Trait callback exits are different:

- they are backend-facing callback boundaries
- they are the outermost fallible entry/exit surfaces of authored DSL
- once a semantic error escapes there, there is no more authored DSL context
  left to handle it
- the next consumer is compiler-generated or backend-generated glue

So the trait callback exit is the place where the compiler must stop treating
 the error as purely abstract semantic flow and start lowering it into the
 target-specific callback result mechanism.

In other words:

- helper boundary: authored caller still exists, so semantic flow can continue
- trait callback boundary: authored caller no longer exists, so lowering must
  take over

## Helper Functions

The current model treats helper functions as semantic-flow zones.

This means:

- helper functions may create `RegseqError`
- helper functions may propagate fallible results with `?`
- helper functions may match, normalize, or reclassify semantic errors
- helper functions do not force backend-specific error materialization just
  because they return a fallible carrier

What matters is not that a helper is fallible, but whether its result escapes
 through a commit point later.

## Trait Callback Functions

The current direction is that Regseq traits are callback/glue contracts, not
 general callable abstraction surfaces.

For the error model, that implies:

- trait callback bodies behave like authored semantic-flow code internally
- trait callback exits are outermost authored fallible boundaries
- direct authored invocation of Regseq trait methods is likely to become a
  separate frontend usage error

This removes one source of confusion from the error design space:

- we do not need to reason about authored code calling trait callback methods
  and then locally handling their errors

## Backend-Native Failure in Authored DSL

The discussion narrowed this point significantly.

From the perspective of what is modeled inside authored DSL:

- backend-native operational failure should enter through fallible intrinsics
- not through Regseq trait callbacks

This is because:

- intrinsics have no authored semantic body
- the compiler has intimate knowledge of how to lower them
- some targets may lower them to real OS-specific mechanisms
- some targets may lower them to trivial/no-op realizations

Trait callbacks are different:

- they are authored callback bodies
- if they return semantic failure, that is whatever semantic error the author
  decided to return

## Why a Flat RegseqError Everywhere Is Too Weak

The discussion returned to an earlier intuition that proved important:

- flattening every fallible intrinsic directly to `RegseqError` loses too much
  local structure

If every fallible operation just returns `RegseqError`, then:

- authors cannot tell which subset of errors a particular intrinsic may really
  produce
- the compiler loses precision at the intrinsic boundary
- local matching becomes less meaningful

This is especially relevant for intrinsic-like operations such as:

- clock enable
- reset sequencing
- polling
- waits

## Current Preferred Refinement: Specialized Error Types

The current preferred refinement is:

1. `RegseqError` remains the only compiler-facing common error enum.
2. Fallible intrinsics may return specialized local error types.
3. Those local error types convert into `RegseqError`.

Examples:

```rust
enum ClockError {
    Busy,
    ResourceUnavailable,
}

impl From<ClockError> for RegseqError { /* ... */ }

fn enable(&self) -> Result<(), ClockError>;
```

and:

```rust
enum PollError {
    Timeout,
}

impl From<PollError> for RegseqError { /* ... */ }

fn until(...) -> Result<(), PollError>;
```

Under this model:

- the intrinsic boundary exposes the smaller valid error set
- helper and callback code can still converge on `RegseqError`
- `?` remains straightforward through `From`

## Why This Is Better Than Two Propagated Carriers

We explored a design with:

- one semantic carrier for `RegseqError`
- one backend-native propagated carrier

The follow-up review found that this did not simplify the model overall.

Main reasons:

- mixed-carrier function bodies become awkward
- `?` interop becomes complicated
- callback-boundary conversion recreates the same ambiguity in another place
- the model becomes harder to learn without clearly improving lowering

So the current direction is:

- keep one common semantic carrier at the shared level
- use specialized local error enums where needed
- do not introduce a second propagated fallible carrier

## What This Means For Intrinsics

The current model implies:

- fallible intrinsics may return specialized local error types
- the compiler promises those values are meaningful inside authored DSL code
- target-specific no-op lowering remains allowed
- a no-op lowering can still return a valid success value or other trivial value
  appropriate to the intrinsic contract

This means the compiler is free to materialize target-specific realizations,
 but the authored DSL still sees a coherent fallible interface.

## Provisional Seed Error Taxonomy

The following table is a useful seed for discussion of the initial
 `RegseqError` space and target-lowering mappings.

It is intentionally **provisional** and should be reviewed before being treated
 as a resolution.

| RegseqError | Linux | Zephyr | Windows (NTSTATUS) | macOS (IOReturn) |
|---|---|---|---|---|
| `InvalidParameter` | `-EINVAL` (`-22`) | `-EINVAL` (`-22`) | `STATUS_INVALID_PARAMETER` (`0xC000000D`) | `kIOReturnBadArgument` (`0xE00002C2`) |
| `OutOfMemory` | `-ENOMEM` (`-12`) | `-ENOMEM` (`-12`) | `STATUS_NO_MEMORY` (`0xC000009A`) | `kIOReturnNoMemory` (`0xE00002BD`) |
| `HardwareError` | `-EIO` (`-5`) | `-EIO` (`-5`) | `STATUS_IO_DEVICE_ERROR` (`0xC0000185`) | `kIOReturnIOError` (`0xE00002BC`) |
| `Timeout` | `-ETIMEDOUT` (`-110`) | `-ETIMEDOUT` (`-116`) | `STATUS_IO_TIMEOUT` (`0xC00000B5`) | `kIOReturnTimeout` (`0xE00002D6`) |
| `Unsupported` | `-EOPNOTSUPP` (`-95`) | `-ENOTSUP` (`-134`) | `STATUS_NOT_SUPPORTED` (`0xC00000BB`) | `kIOReturnUnsupported` (`0xE00002BE`) |
| `DeviceBusy` | `-EBUSY` (`-16`) | `-EBUSY` (`-16`) | `STATUS_DEVICE_BUSY` (`0xC00000E5`) | `kIOReturnBusy` (`0xE00002D8`) |
| `PermissionDenied` | `-EPERM` (`-1`) | `-EPERM` (`-1`) | `STATUS_ACCESS_DENIED` (`0xC0000022`) | `kIOReturnNotPermitted` (`0xE00002C1`) |
| `DeviceNotFound` | `-ENODEV` (`-19`) | `-ENODEV` (`-19`) | `STATUS_NO_SUCH_DEVICE` (`0xC000000E`) | `kIOReturnNotFound` (`0xE00002C0`) |
| `GenericFailure` | `-EIO` (`-5`) | `-EIO` (`-5`) | `STATUS_UNSUCCESSFUL` (`0xC0000001`) | `kIOReturnIOError` (`0xE00002BC`) |

### Notes on the seed table

- The symbolic target constants matter more than the numeric literals. Backends
  should lower to symbolic forms, not hard-coded integers.
- This table is a seeding tool for the discussion, not a final statement that
  every target must expose every row identically.
- Several names may still need normalization before final resolution. In
  particular, the discussion has also used names such as `InvalidArgument`,
  `Busy`, and `DeviceNotPresent`.
- `GenericFailure` is the weakest row conceptually. It may remain as a backend
  fallback, but it should not become a substitute for more precise semantic
  categories unless review confirms that need.
- The table says nothing yet about which intrinsic-local error enums should
  convert into which rows. That remains a separate design step.

## What This Means For Wrappers

Wrappers are no longer the primary conceptual boundary.

The cleaner rule is:

- wrappers are either ergonomic sugar over intrinsics
- or pure authored helper surfaces

If a wrapper is fallible because it fronts an intrinsic, then the real local
 error contract should be determined by the underlying intrinsic family.

That suggests the error model should be designed primarily around:

- intrinsic-local error types
- callback-exit `RegseqError` unification

rather than around wrapper-specific error semantics in the abstract.

## Boilerplate Reduction

The discussion also surfaced a likely ergonomic improvement:

- provide a derive-style helper macro that generates
  `From<LocalError> for RegseqError`

Example shape:

```rust
#[derive(RegseqErrorMap)]
pub enum ClockError {
    Busy,
    #[regseq_error(to = RegseqError::ResourceUnavailable)]
    MissingClock,
}
```

This is not a core semantic decision, but it is a good ergonomic follow-on once
 the local-error-type direction is accepted.

## Things That Still Look Stable

These points appear stable enough to treat as current working decisions:

- `RegseqError` is semantic and compiler-facing
- no `std::error::Error` requirement
- no `Display` requirement
- helper functions are semantic-flow zones
- trait callback exits are authored commit points
- backend-native operational failure in authored DSL should enter through
  intrinsics
- two propagated carriers are not preferred
- intrinsic-local error types converting into `RegseqError` are the current
  best refinement

## Open Questions

These still need explicit closure:

1. What is the initial `RegseqError` variant set?
2. Which intrinsic families get their own local error enums?
3. Which wrappers are true intrinsic sugar and which are just authored helpers?
4. Should helper functions be allowed to return local intrinsic-specific error
   types freely, or should shared callback/helper composition strongly prefer
   normalization to `RegseqError`?
5. What exact authored carrier syntax do we want:
   - `core::result::Result<T, RegseqError>`
   - a custom `RegseqResult<T>`
6. What should the derive/helper macro for local error mapping look like?

## Intended Next Step

This synthesis should now be reviewed critically.

The next round of feedback should test:

- whether the intrinsic-local error type direction really solves the earlier
  wrapper-boundary concerns
- whether the commit-point model is stated clearly enough
- whether the current stable points should be promoted into an explicit
  resolution
